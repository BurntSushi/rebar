use std::io::Write;

use {
    anyhow::Context,
    bstr::ByteSlice,
    lexopt::{Arg, ValueExt},
};

use crate::ffi::{is_jit_available, Options, Regex};

mod ffi;

fn main() -> anyhow::Result<()> {
    let mut p = lexopt::Parser::from_env();
    let engine = match p.next()? {
        None => anyhow::bail!("missing engine name"),
        Some(Arg::Value(v)) => v.string().context("<engine>")?,
        Some(arg) => {
            return Err(
                anyhow::Error::from(arg.unexpected()).context("<engine>")
            );
        }
    };
    anyhow::ensure!(
        engine == "interp" || engine == "jit",
        "unrecognized engine '{}'",
        engine,
    );
    let jit = engine == "jit";
    if jit && !is_jit_available() {
        anyhow::bail!("JIT engine unavailable because JIT is not enabled");
    }
    let (mut quiet, mut version) = (false, false);
    while let Some(arg) = p.next()? {
        match arg {
            Arg::Short('h') | Arg::Long("help") => {
                anyhow::bail!("main <engine> [--version | --quiet]")
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
        let v = crate::ffi::version();
        writeln!(std::io::stdout(), "{}", v)?;
        return Ok(());
    }
    let b = klv::Benchmark::read(std::io::stdin())
        .context("failed to read KLV data from <stdin>")?;
    let samples = match b.model.as_str() {
        "compile" => model_compile(&b, jit)?,
        "count" => model_count(&b, &compile(&b, jit)?)?,
        "count-spans" => model_count_spans(&b, &compile(&b, jit)?)?,
        "count-captures" => model_count_captures(&b, &compile(&b, jit)?)?,
        "grep" => model_grep(&b, &compile(&b, jit)?)?,
        "grep-captures" => model_grep_captures(&b, &compile(&b, jit)?)?,
        "regex-redux" => model_regex_redux(&b, jit)?,
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

fn model_compile(
    b: &klv::Benchmark,
    jit: bool,
) -> anyhow::Result<Vec<timer::Sample>> {
    let haystack = &*b.haystack;
    timer::run_and_count(
        b,
        |re: Regex| {
            let mut md = re.create_match_data_for_matches_only();
            Ok(re.try_find_iter(haystack, &mut md).count())
        },
        || compile(b, jit),
    )
}

fn model_count(
    b: &klv::Benchmark,
    re: &Regex,
) -> anyhow::Result<Vec<timer::Sample>> {
    let haystack = &*b.haystack;
    let mut md = re.create_match_data_for_matches_only();
    timer::run(b, || {
        let mut count = 0;
        for result in re.try_find_iter(haystack, &mut md) {
            result?;
            count += 1;
        }
        Ok(count)
    })
}

fn model_count_spans(
    b: &klv::Benchmark,
    re: &Regex,
) -> anyhow::Result<Vec<timer::Sample>> {
    let haystack = &*b.haystack;
    let mut md = re.create_match_data_for_matches_only();
    timer::run(b, || {
        let mut sum = 0;
        for result in re.try_find_iter(haystack, &mut md) {
            let (start, end) = result?;
            sum += end - start;
        }
        Ok(sum)
    })
}

fn model_count_captures(
    b: &klv::Benchmark,
    re: &Regex,
) -> anyhow::Result<Vec<timer::Sample>> {
    let haystack = &*b.haystack;
    let mut md = re.create_match_data();
    timer::run(b, || {
        let mut at = 0;
        let mut count = 0;
        while let Some((_, end)) = {
            re.try_find(haystack, at, haystack.len(), &mut md)?;
            md.get_match()
        } {
            for i in 0..md.group_len() {
                if md.get_group(i).is_some() {
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
    let mut md = re.create_match_data_for_matches_only();
    timer::run(b, || {
        let mut count = 0;
        for line in haystack.lines() {
            if re.try_find(line, 0, line.len(), &mut md)? {
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
    let mut md = re.create_match_data();
    timer::run(b, || {
        let mut count = 0;
        for line in haystack.lines() {
            let mut at = 0;
            while let Some((_, end)) = {
                re.try_find(line, at, line.len(), &mut md)?;
                md.get_match()
            } {
                for i in 0..md.group_len() {
                    if md.get_group(i).is_some() {
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
    jit: bool,
) -> anyhow::Result<Vec<timer::Sample>> {
    let haystack = b.haystack_str()?;
    let compile = |pattern: &str| -> anyhow::Result<regexredux::RegexFn> {
        let re = Regex::new(pattern, options(b, jit))?;
        let mut md = re.create_match_data_for_matches_only();
        let find = move |h: &str| {
            re.try_find(h.as_bytes(), 0, h.len(), &mut md)?;
            Ok(md.get_match())
        };
        Ok(Box::new(find))
    };
    timer::run(b, || regexredux::generic(haystack, compile))
}

fn compile(b: &klv::Benchmark, jit: bool) -> anyhow::Result<Regex> {
    let re = Regex::new(&b.regex.one()?, options(b, jit))?;
    Ok(re)
}

fn options(b: &klv::Benchmark, jit: bool) -> Options {
    Options { jit, ucp: b.regex.unicode, caseless: b.regex.case_insensitive }
}
