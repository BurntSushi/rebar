use std::io::Write;

use {
    aho_corasick::{packed, AhoCorasick, AhoCorasickKind, MatchKind},
    anyhow::Context,
    bstr::ByteSlice,
    lexopt::{Arg, ValueExt},
};

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
    let kind = match &*engine {
        "dfa" => AhoCorasickKind::DFA,
        "nfa" => AhoCorasickKind::ContiguousNFA,
        "teddy" => return main_teddy(&b, quiet),
        _ => anyhow::bail!("unrecognized engine '{}'", engine),
    };
    let samples = match b.model.as_str() {
        "compile" => model_compile_ac(&b, kind)?,
        "count" => model_count_ac(&b, &compile_ac(&b, kind)?)?,
        "count-spans" => model_count_spans_ac(&b, &compile_ac(&b, kind)?)?,
        "grep" => model_grep_ac(&b, &compile_ac(&b, kind)?)?,
        _ => anyhow::bail!("unsupported benchmark model '{}'", b.model),
    };
    if !quiet {
        let mut stdout = std::io::stdout().lock();
        for s in samples.iter() {
            writeln!(stdout, "{},{}", s.duration.as_nanos(), s.count)?;
        }
    }
    Ok(())
}

fn main_teddy(b: &klv::Benchmark, quiet: bool) -> anyhow::Result<()> {
    let samples = match b.model.as_str() {
        "compile" => model_compile_teddy(&b)?,
        "count" => model_count_teddy(&b, &compile_teddy(&b)?)?,
        "count-spans" => model_count_spans_teddy(&b, &compile_teddy(&b)?)?,
        "grep" => model_grep_teddy(&b, &compile_teddy(&b)?)?,
        _ => anyhow::bail!("unsupported benchmark model '{}'", b.model),
    };
    if !quiet {
        let mut stdout = std::io::stdout().lock();
        for s in samples.iter() {
            writeln!(stdout, "{},{}", s.duration.as_nanos(), s.count)?;
        }
    }
    Ok(())
}

fn model_compile_ac(
    b: &klv::Benchmark,
    kind: AhoCorasickKind,
) -> anyhow::Result<Vec<timer::Sample>> {
    let haystack = &*b.haystack;
    timer::run_and_count(
        b,
        |re: AhoCorasick| Ok(re.find_iter(haystack).count()),
        || compile_ac(b, kind),
    )
}

fn model_count_ac(
    b: &klv::Benchmark,
    re: &AhoCorasick,
) -> anyhow::Result<Vec<timer::Sample>> {
    let haystack = &*b.haystack;
    timer::run(b, || Ok(re.find_iter(haystack).count()))
}

fn model_count_spans_ac(
    b: &klv::Benchmark,
    re: &AhoCorasick,
) -> anyhow::Result<Vec<timer::Sample>> {
    let haystack = &*b.haystack;
    timer::run(b, || Ok(re.find_iter(haystack).map(|m| m.len()).sum()))
}

fn model_grep_ac(
    b: &klv::Benchmark,
    re: &AhoCorasick,
) -> anyhow::Result<Vec<timer::Sample>> {
    let haystack = &*b.haystack;
    timer::run(b, || {
        let mut count = 0;
        for line in haystack.lines() {
            if re.is_match(line) {
                count += 1;
            }
        }
        Ok(count)
    })
}

fn compile_ac(
    b: &klv::Benchmark,
    kind: AhoCorasickKind,
) -> anyhow::Result<AhoCorasick> {
    anyhow::ensure!(
        !(b.regex.unicode && b.regex.case_insensitive),
        "rust/aho-corasick engines are incompatible with 'unicode = true' and \
         'case-insensitive = true'"
    );
    let ac = AhoCorasick::builder()
        .kind(Some(kind))
        .match_kind(MatchKind::LeftmostFirst)
        .ascii_case_insensitive(b.regex.case_insensitive)
        .prefilter(false)
        .build(&b.regex.patterns)?;
    Ok(ac)
}

fn model_compile_teddy(
    b: &klv::Benchmark,
) -> anyhow::Result<Vec<timer::Sample>> {
    let haystack = &*b.haystack;
    timer::run_and_count(
        b,
        |re: packed::Searcher| Ok(re.find_iter(haystack).count()),
        || compile_teddy(b),
    )
}

fn model_count_teddy(
    b: &klv::Benchmark,
    re: &packed::Searcher,
) -> anyhow::Result<Vec<timer::Sample>> {
    let haystack = &*b.haystack;
    timer::run(b, || Ok(re.find_iter(haystack).count()))
}

fn model_count_spans_teddy(
    b: &klv::Benchmark,
    re: &packed::Searcher,
) -> anyhow::Result<Vec<timer::Sample>> {
    let haystack = &*b.haystack;
    timer::run(b, || Ok(re.find_iter(haystack).map(|m| m.len()).sum()))
}

fn model_grep_teddy(
    b: &klv::Benchmark,
    re: &packed::Searcher,
) -> anyhow::Result<Vec<timer::Sample>> {
    let haystack = &*b.haystack;
    timer::run(b, || {
        let mut count = 0;
        for line in haystack.lines() {
            if re.find(line).is_some() {
                count += 1;
            }
        }
        Ok(count)
    })
}

fn compile_teddy(b: &klv::Benchmark) -> anyhow::Result<packed::Searcher> {
    anyhow::ensure!(
        !b.regex.case_insensitive,
        "rust/aho-corasick/teddy engine is incompatible with \
         'case-insensitive = true'"
    );
    let searcher = packed::Config::new()
        .match_kind(packed::MatchKind::LeftmostFirst)
        .builder()
        .extend(&b.regex.patterns)
        .build()
        .ok_or_else(|| anyhow::anyhow!("failed to build Teddy searcher"))?;
    Ok(searcher)
}
