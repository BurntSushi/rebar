use std::io::Write;

use {
    anyhow::Context,
    bstr::ByteSlice,
    vectorscan::{
        database::*, expression::*, flags::*, matchers::*}
    ,
    lexopt::Arg,
};

fn main() -> anyhow::Result<()> {
    let mut p = lexopt::Parser::from_env();
    let (mut quiet, mut version) = (false, false);
    while let Some(arg) = p.next()? {
        match arg {
            Arg::Short('h') | Arg::Long("help") => {
                anyhow::bail!("main [--version | --quiet]")
            }
            Arg::Short('q') | Arg::Long("quiet") => {
                quiet = true;
            }
            Arg::Long("version") => {
                version = true;
            }
            _ => return Err(arg.unexpected().into()),
        }
    }
    if version {
        let version = vectorscan::vectorscan_version().to_string_lossy();
        writeln!(std::io::stdout(), "{}", version)?;
        return Ok(());
    }
    let b = klv::Benchmark::read(std::io::stdin())
        .context("failed to read KLV data from <stdin>")?;
    let samples = match b.model.as_str() {
        "compile" => model_compile(&b)?,
        "count" => model_count(&b)?,
        "count-spans" => model_count_spans(&b)?,
        "grep" => model_grep(&b)?,
        "regex-redux" => model_regex_redux(&b)?,
        _ => anyhow::bail!("unrecognized benchmark model '{}'", b.model),
    };
    if !quiet {
        let mut stdout = std::io::stdout().lock();
        for s in samples.iter() {
            writeln!(stdout, "{},{}", s.duration.as_nanos(), s.count)?;
        }
    }
    Ok(())
}

fn model_compile(b: &klv::Benchmark) -> anyhow::Result<Vec<timer::Sample>> {
    let haystack = &*b.haystack;
    timer::run_and_count(
        b,
        |re: vectorscan::database::Database| {
            let mut scratch = re.allocate_scratch()?;
            let mut count = 0;
            scratch.scan_sync(&re, haystack.into(), |Match { .. }| {
                count += 1;
                MatchResult::Continue
            })?;
            Ok(count)
        },
        // Does SOM have an impact on compilation times..? same as hyperscan
        || compile(b, Flags::NONE),
    )
}

fn model_count(b: &klv::Benchmark) -> anyhow::Result<Vec<timer::Sample>> {
    let haystack = &*b.haystack;
    // If all we need to do is count matches, we don't care about SOM.
    let re = compile(b, Flags::NONE)?;
    let mut scratch = re.allocate_scratch()?;
    timer::run(b, || {
        let mut count = 0;
        scratch.scan_sync(&re, haystack.into(), |Match { .. }| {
            count += 1;
            MatchResult::Continue
        })?;
        Ok(count)
    })
}

fn model_count_spans(
    b: &klv::Benchmark,
) -> anyhow::Result<Vec<timer::Sample>> {
    let haystack = &*b.haystack;
    // In order to compute the length of a match span, we need the start of the
    // match, but with vectorscan crate we only get the match itself.
    let re = compile(b, Flags::SOM_LEFTMOST)?;
    let mut scratch = re.allocate_scratch()?;
    timer::run(b, || {
        let mut sum = 0;
        scratch.scan_sync(&re, haystack.into(), |Match { source, .. }| {
            sum += source.as_slice().len();
            MatchResult::Continue
        })?;
        Ok(sum)
    })
}

fn model_grep(b: &klv::Benchmark) -> anyhow::Result<Vec<timer::Sample>> {
    use vectorscan::error::VectorscanRuntimeError;
    let haystack = &*b.haystack;
    // We don't need SOM handling to detect if a line matched.
    let re = compile(b, Flags::NONE)?;
    let mut scratch = re.allocate_scratch()?;
    timer::run(b, || {
        let mut count = 0;
        for line in haystack.lines() {
            // Apparently the 'scan' API returns an error if we tell searching
            // to stop, which seems... strange. So we ignore errors for now,
            // but probably we should check that if an error is returned, it's
            // the "search was terminated" error and not something else.
            
            //With vectorscan crate if we ignore the error, tests will fail
            //checking if the error is the expected one gets the job done
            //but it impacts the timers
            
            let ret = scratch.scan_sync(&re, line.into(), |Match { .. }| {
                count += 1;
                MatchResult::CeaseMatching
            });

            match ret {
                Ok(_) => (), // No error, continue
                Err(e) => match e {
                    VectorscanRuntimeError::ScanTerminated => {
                        // Expected termination, ignore and continue
                    },
                    _ => return Err(e.into()), // Unexpected error, return it
                },
            }
        }
        Ok(count)
    })
}

fn model_regex_redux(
    b: &klv::Benchmark,
) -> anyhow::Result<Vec<timer::Sample>> {
    use vectorscan::error::VectorscanRuntimeError;
    let haystack = b.haystack_str()?;
    let compile = |p: &str| -> anyhow::Result<regexredux::RegexFn> {
        let re: vectorscan::database::Database = 
            pattern(p)?.compile(Flags::SOM_LEFTMOST,Mode::BLOCK)?;
        let mut scratch = re.allocate_scratch()?;
        let find = move |h: &str| {
            let mut m: Option<(usize, usize)> = None;
            // Apparently the 'scan' API returns an error if we tell searching
            // to stop, which seems... strange. So we ignore errors for now,
            // but probably we should check that if an error is returned, it's
            // the "search was terminated" error and not something else.

            //With vectorscan crate if we ignore the error, tests will fail
            //checking if the error is the expected one gets the job done
            //but it impacts the timers
            
            //vectorscan crate might have to return start of the match and end of the match like the hyperscan one
            //It gives wrong output
            let ret = scratch.scan_sync(&re, h.into(), |Match { source, .. }| {
                m = Some((0 as usize, source.as_slice().len() as usize));
                MatchResult::CeaseMatching
            });    
            
            
            match ret {
                Ok(_) => (), // No error, continue
                Err(e) => match e {
                    VectorscanRuntimeError::ScanTerminated => {
                        // Expected termination, ignore and continue
                    },
                    _ => return Err(e.into()), // Unexpected error, return it
                },
            }
            Ok(m)
        };
        Ok(Box::new(find))
    };
    timer::run(b, || regexredux::generic(haystack, compile))
}

fn compile(
    b: &klv::Benchmark,
    additional_flags: Flags,
) -> anyhow::Result<Database> {
    let mut exprs = Vec::new();
    let mut the_flags = Vec::new();

    for p in b.regex.patterns.iter() {
        let flags = bench_flags(b)? | additional_flags;
        let expr = p.parse::<Expression>()?;
        exprs.push(expr);
        the_flags.push(flags);
    }

    let db = ExpressionSet::from_exprs(&exprs)
        .with_flags(the_flags)
        .compile(Mode::BLOCK)?;

    Ok(db)
}

fn pattern(
    pat: &str,
) -> anyhow::Result<vectorscan::expression::Expression> {
    let pattern = pat.parse::<vectorscan::expression::Expression>()?;
    Ok(pattern)
}

fn bench_flags(b: &klv::Benchmark) -> anyhow::Result<Flags> {
    let mut f = Flags::NONE;
    if b.regex.unicode {
        //Same for the vectorscan
        // Hyperscan docs for the HS_FLAG_UTF8 option state[1]:
        //
        // > The results of scanning invalid UTF-8 sequences with a Hyperscan
        // > library that has been compiled with one or more patterns using
        // > this flag are undefined.
        //
        // For the HS_FLAG_UCP option, they also state:
        //
        // > It is only meaningful in conjunction with HS_FLAG_UTF8.
        //
        // So if Unicode mode is enabled, we have to enable both UTF-8 and UCP
        // modes. AND we need to ensure that the haystack is valid UTF-8.
        //
        // From my recollection this actually matches what PCRE2 required
        // before 10.34 or so, at which point, they introduced the
        // PCRE2_MATCH_INVALID_UTF option to enable matching on invalid UTF-8
        // with Unicode mode enabled. But Hyperscan doesn't seem to support
        // this mode.
        //
        // [1]: https://intel.github.io/hyperscan/dev-reference/api_constants.html
        let _ = b.haystack_str()?;
        f |= Flags::UCP;
        f |= Flags::UTF8;
    }
    if b.regex.case_insensitive {
        f |= Flags::CASELESS;
    }
    Ok(f)
}