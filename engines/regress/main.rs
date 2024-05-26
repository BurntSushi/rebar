use std::io::Write;

use {
    anyhow::Context,
    lexopt::Arg,
    regress::{Flags, Regex},
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
        writeln!(std::io::stdout(), "{}", env!("CARGO_PKG_VERSION"))?;
        return Ok(());
    }
    let b = klv::Benchmark::read(std::io::stdin())
        .context("failed to read KLV data from <stdin>")?;
    let samples = match b.model.as_str() {
        "compile" => model_compile(&b)?,
        "count" => model_count(&b, &compile(&b)?)?,
        "count-spans" => model_count_spans(&b, &compile(&b)?)?,
        "count-captures" => model_count_captures(&b, &compile(&b)?)?,
        "grep" => model_grep(&b, &compile(&b)?)?,
        "grep-captures" => model_grep_captures(&b, &compile(&b)?)?,
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
    let haystack = b.haystack_str()?;
    timer::run_and_count(
        b,
        |re: Regex| Ok(re.find_iter(haystack).count()),
        || compile(b),
    )
}

fn model_count(
    b: &klv::Benchmark,
    re: &Regex,
) -> anyhow::Result<Vec<timer::Sample>> {
    let haystack = b.haystack_str()?;
    timer::run(b, || Ok(re.find_iter(haystack).count()))
}

fn model_count_spans(
    b: &klv::Benchmark,
    re: &Regex,
) -> anyhow::Result<Vec<timer::Sample>> {
    let haystack = b.haystack_str()?;
    timer::run(b, || {
        Ok(re.find_iter(haystack).map(|m| m.end() - m.start()).sum())
    })
}

fn model_count_captures(
    b: &klv::Benchmark,
    re: &Regex,
) -> anyhow::Result<Vec<timer::Sample>> {
    let haystack = b.haystack_str()?;
    timer::run(b, || {
        let mut count = 0;
        for m in re.find_iter(haystack) {
            // +1 to count the implicit group.
            count += 1 + m.captures.iter().filter(|c| c.is_some()).count();
        }
        Ok(count)
    })
}

fn model_grep(
    b: &klv::Benchmark,
    re: &Regex,
) -> anyhow::Result<Vec<timer::Sample>> {
    let haystack = b.haystack_str()?;
    timer::run(b, || {
        let mut count = 0;
        // NOTE: This is actually using std's line iteration logic instead
        // of bstr's because haystack is a &str here and not a &[u8]. This
        // technically means we aren't comparing apples-to-oranges here, but
        // the benchmark model definition explicitly permits these sorts of
        // cases because it reflects reality.
        for line in haystack.lines() {
            if re.find(line).is_some() {
                count += 1;
            }
        }
        Ok(count)
    })
}

fn model_grep_captures(
    b: &klv::Benchmark,
    re: &Regex,
) -> anyhow::Result<Vec<timer::Sample>> {
    let haystack = b.haystack_str()?;
    timer::run(b, || {
        let mut count = 0;
        // NOTE: This is actually using std's line iteration logic instead
        // of bstr's because haystack is a &str here and not a &[u8]. This
        // technically means we aren't comparing apples-to-oranges here, but
        // the benchmark model definition explicitly permits these sorts of
        // cases because it reflects reality.
        for line in haystack.lines() {
            for m in re.find_iter(line) {
                // +1 to count the implicit group
                count += 1 + m.captures.iter().filter(|c| c.is_some()).count();
            }
        }
        Ok(count)
    })
}

fn model_regex_redux(
    b: &klv::Benchmark,
) -> anyhow::Result<Vec<timer::Sample>> {
    let haystack = b.haystack_str()?;
    let compile = |pattern: &str| -> anyhow::Result<regexredux::RegexFn> {
        let re = Regex::with_flags(pattern, flags(b))?;
        let find = move |h: &str| Ok(re.find(h).map(|m| (m.start(), m.end())));
        Ok(Box::new(find))
    };
    timer::run(b, || regexredux::generic(haystack, compile))
}

fn compile(b: &klv::Benchmark) -> anyhow::Result<Regex> {
    Ok(Regex::with_flags(&b.regex.one()?, flags(b))?)
}

fn flags(b: &klv::Benchmark) -> Flags {
    Flags {
        icase: b.regex.case_insensitive,
        multiline: false,
        dot_all: false,
        no_opt: false,
        unicode: b.regex.unicode,
        unicode_sets: false,
    }
}
