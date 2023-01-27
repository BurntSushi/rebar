use std::io::Write;

use {anyhow::Context, bstr::ByteSlice, lexopt::Arg};

use crate::ffi::{Options, Regex};

mod ffi;
mod version;

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
        writeln!(std::io::stdout(), "{}", crate::version::VERSION)?;
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
    let haystack = &*b.haystack;
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
    let haystack = &*b.haystack;
    timer::run(b, || Ok(re.find_iter(haystack).count()))
}

fn model_count_spans(
    b: &klv::Benchmark,
    re: &Regex,
) -> anyhow::Result<Vec<timer::Sample>> {
    let haystack = &*b.haystack;
    timer::run(b, || Ok(re.find_iter(haystack).map(|(s, e)| e - s).sum()))
}

fn model_count_captures(
    b: &klv::Benchmark,
    re: &Regex,
) -> anyhow::Result<Vec<timer::Sample>> {
    let haystack = &*b.haystack;
    let mut caps = re.create_captures();
    timer::run(b, || {
        let mut at = 0;
        let mut count = 0;
        while let Some((_, end)) = {
            re.captures(haystack, at, haystack.len(), &mut caps);
            caps.get_match()
        } {
            for i in 0..caps.group_len() {
                if caps.get_group(i).is_some() {
                    count += 1;
                }
            }
            // Benchmark definition says we may assume empty matches are
            // impossible.
            at = end;
        }
        Ok(count)
    })
}

fn model_grep(
    b: &klv::Benchmark,
    re: &Regex,
) -> anyhow::Result<Vec<timer::Sample>> {
    let haystack = &*b.haystack;
    timer::run(b, || {
        let mut count = 0;
        for line in haystack.lines() {
            if re.is_match(line, 0, line.len()) {
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
    let haystack = &*b.haystack;
    let mut caps = re.create_captures();
    timer::run(b, || {
        let mut count = 0;
        for line in haystack.lines() {
            let mut at = 0;
            while let Some((_, end)) = {
                re.captures(line, at, line.len(), &mut caps);
                caps.get_match()
            } {
                for i in 0..caps.group_len() {
                    if caps.get_group(i).is_some() {
                        count += 1;
                    }
                }
                // Benchmark definition says we may assume empty matches are
                // impossible.
                at = end;
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
        let re = Regex::new(pattern, options(b))?;
        let find = move |h: &str| Ok(re.find(h.as_bytes(), 0, h.len()));
        Ok(Box::new(find))
    };
    timer::run(b, || regexredux::generic(haystack, compile))
}

fn compile(b: &klv::Benchmark) -> anyhow::Result<Regex> {
    Regex::new(&b.regex.one()?, options(b))
}

fn options(b: &klv::Benchmark) -> Options {
    Options {
        utf8: b.regex.unicode,
        case_sensitive: !b.regex.case_insensitive,
    }
}
