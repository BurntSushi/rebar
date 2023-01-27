use std::io::Write;

use {anyhow::Context, bstr::ByteSlice, lexopt::Arg, memchr::memmem::Finder};

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
        "grep" => model_grep(&b, &compile(&b)?)?,
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

fn model_compile(b: &klv::Benchmark) -> anyhow::Result<Vec<timer::Sample>> {
    let haystack = &*b.haystack;
    timer::run_and_count(
        b,
        |f: Finder| Ok(f.find_iter(haystack).count()),
        || compile(b),
    )
}

fn model_count(
    b: &klv::Benchmark,
    f: &Finder,
) -> anyhow::Result<Vec<timer::Sample>> {
    let haystack = &*b.haystack;
    timer::run(b, || Ok(f.find_iter(haystack).count()))
}

fn model_count_spans(
    b: &klv::Benchmark,
    f: &Finder,
) -> anyhow::Result<Vec<timer::Sample>> {
    let haystack = &*b.haystack;
    timer::run(b, || Ok(f.find_iter(haystack).map(|_| f.needle().len()).sum()))
}

fn model_grep(
    b: &klv::Benchmark,
    f: &Finder,
) -> anyhow::Result<Vec<timer::Sample>> {
    let haystack = &*b.haystack;
    timer::run(b, || {
        let mut count = 0;
        for line in haystack.lines() {
            if f.find(line).is_some() {
                count += 1;
            }
        }
        Ok(count)
    })
}

fn compile(b: &klv::Benchmark) -> anyhow::Result<Finder> {
    anyhow::ensure!(
        !b.regex.case_insensitive,
        "rust/memchr/memmem engine is incompatible with case insensitive mode",
    );
    Ok(Finder::new(b.regex.one()?.as_bytes()))
}
