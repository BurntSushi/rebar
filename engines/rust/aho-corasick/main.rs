use std::io::Write;

use {
    aho_corasick::{AhoCorasick, AhoCorasickKind, MatchKind},
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
    let kind = match &*engine {
        "dfa" => AhoCorasickKind::DFA,
        "nfa" => AhoCorasickKind::ContiguousNFA,
        _ => anyhow::bail!("unrecognized engine '{}'", engine),
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
    let samples = match b.model.as_str() {
        "compile" => model_compile(&b, kind)?,
        "count" => model_count(&b, &compile(&b, kind)?)?,
        "count-spans" => model_count_spans(&b, &compile(&b, kind)?)?,
        "grep" => model_grep(&b, &compile(&b, kind)?)?,
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

fn model_compile(
    b: &klv::Benchmark,
    kind: AhoCorasickKind,
) -> anyhow::Result<Vec<timer::Sample>> {
    let haystack = &*b.haystack;
    timer::run_and_count(
        b,
        |re: AhoCorasick| Ok(re.find_iter(haystack).count()),
        || compile(b, kind),
    )
}

fn model_count(
    b: &klv::Benchmark,
    re: &AhoCorasick,
) -> anyhow::Result<Vec<timer::Sample>> {
    let haystack = &*b.haystack;
    timer::run(b, || Ok(re.find_iter(haystack).count()))
}

fn model_count_spans(
    b: &klv::Benchmark,
    re: &AhoCorasick,
) -> anyhow::Result<Vec<timer::Sample>> {
    let haystack = &*b.haystack;
    timer::run(b, || Ok(re.find_iter(haystack).map(|m| m.span().len()).sum()))
}

fn model_grep(
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

fn compile(
    b: &klv::Benchmark,
    kind: AhoCorasickKind,
) -> anyhow::Result<AhoCorasick> {
    anyhow::ensure!(
        !(b.regex.unicode && b.regex.case_insensitive),
        "rust/aho-corasick engine is incompatible with 'unicode = true' and \
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
