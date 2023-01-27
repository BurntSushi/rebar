use std::io::Write;

use {
    anyhow::Context,
    bstr::ByteSlice,
    hyperscan::{
        BlockDatabase, Builder, Matching, Pattern, PatternFlags, Patterns,
    },
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
        let version = hyperscan::version_str().to_string_lossy();
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
        |re: BlockDatabase| {
            let scratch = re.alloc_scratch()?;
            let mut count = 0;
            re.scan(haystack, &scratch, |_id, _from, _to, _flags| {
                count += 1;
                Matching::Continue
            })?;
            Ok(count)
        },
        // Does SOM have an impact on compilation times..?
        || compile(b, PatternFlags::empty()),
    )
}

fn model_count(b: &klv::Benchmark) -> anyhow::Result<Vec<timer::Sample>> {
    let haystack = &*b.haystack;
    // If all we need to do is count matches, we don't care about SOM.
    let re = compile(b, PatternFlags::empty())?;
    let scratch = re.alloc_scratch()?;
    timer::run(b, || {
        let mut count = 0;
        re.scan(haystack, &scratch, |_id, _from, _to, _flags| {
            count += 1;
            Matching::Continue
        })?;
        Ok(count)
    })
}

fn model_count_spans(
    b: &klv::Benchmark,
) -> anyhow::Result<Vec<timer::Sample>> {
    let haystack = &*b.haystack;
    // In order to compute the length of a match span, we need the start of the
    // match, so we ask Hyperscan to compute it.
    let re = compile(b, PatternFlags::SOM_LEFTMOST)?;
    let scratch = re.alloc_scratch()?;
    timer::run(b, || {
        let mut sum = 0;
        re.scan(haystack, &scratch, |_id, from, to, _flags| {
            sum += (to as usize) - (from as usize);
            Matching::Continue
        })?;
        Ok(sum)
    })
}

fn model_grep(b: &klv::Benchmark) -> anyhow::Result<Vec<timer::Sample>> {
    let haystack = &*b.haystack;
    // We don't need SOM handling to detect if a line matched.
    let re = compile(b, PatternFlags::empty())?;
    let scratch = re.alloc_scratch()?;
    timer::run(b, || {
        let mut count = 0;
        for line in haystack.lines() {
            // Apparently the 'scan' API returns an error if we tell searching
            // to stop, which seems... strange. So we ignore errors for now,
            // but probably we should check that if an error is returned, it's
            // the "search was terminated" error and not something else.
            let _ = re.scan(line, &scratch, |_id, _from, _to, _flags| {
                count += 1;
                Matching::Terminate
            });
        }
        Ok(count)
    })
}

fn model_regex_redux(
    b: &klv::Benchmark,
) -> anyhow::Result<Vec<timer::Sample>> {
    let haystack = b.haystack_str()?;
    let compile = |p: &str| -> anyhow::Result<regexredux::RegexFn> {
        let re: BlockDatabase =
            pattern(b, p, PatternFlags::SOM_LEFTMOST)?.build()?;
        let scratch = re.alloc_scratch()?;
        let find = move |h: &str| {
            let mut m: Option<(usize, usize)> = None;
            // Apparently the 'scan' API returns an error if we tell searching
            // to stop, which seems... strange. So we ignore errors for now,
            // but probably we should check that if an error is returned, it's
            // the "search was terminated" error and not something else.
            let _ = re.scan(h, &scratch, |_id, from, to, _flags| {
                m = Some((from as usize, to as usize));
                Matching::Terminate
            });
            Ok(m)
        };
        Ok(Box::new(find))
    };
    timer::run(b, || regexredux::generic(haystack, compile))
}

fn compile(
    b: &klv::Benchmark,
    additional_flags: PatternFlags,
) -> anyhow::Result<BlockDatabase> {
    let mut patterns = Patterns(vec![]);
    for p in b.regex.patterns.iter() {
        patterns.0.push(pattern(b, p, additional_flags)?);
    }
    let re = patterns.build()?;
    Ok(re)
}

fn pattern(
    b: &klv::Benchmark,
    pat: &str,
    additional_flags: PatternFlags,
) -> anyhow::Result<Pattern> {
    let flags = bench_flags(b)? | additional_flags;
    let pattern = Pattern::with_flags(pat, flags)?;
    Ok(pattern)
}

fn bench_flags(b: &klv::Benchmark) -> anyhow::Result<PatternFlags> {
    let mut f = PatternFlags::empty();
    if b.regex.unicode {
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
        f |= PatternFlags::UCP;
        f |= PatternFlags::UTF8;
    }
    if b.regex.case_insensitive {
        f |= PatternFlags::CASELESS;
    }
    Ok(f)
}
