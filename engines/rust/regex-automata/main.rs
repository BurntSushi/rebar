use std::io::Write;

use {
    anyhow::Context,
    lexopt::{Arg, ValueExt},
};

mod model;
mod new;

/// A list of valid engine names supported by this tool. This list is used to
/// validate that the engine name provided is valid.
const ENGINES: &[&str] = &[
    "backtrack",
    "dense",
    "hybrid",
    "meta",
    "nfa",
    "onepass",
    "pikevm",
    "sparse",
];

/// Since this runner has a lot of engines (all of the regex crate's internal
/// engines), we bundle up the engine name with the benchmark config so we
/// can pass it around more easily.
#[derive(Debug)]
struct Config {
    b: klv::Benchmark,
    engine: String,
}

fn main() -> anyhow::Result<()> {
    env_logger::init();

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
        ENGINES.contains(&&*engine),
        "unrecognized engine '{}'",
        engine,
    );
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
    let c = Config { b, engine };
    let samples = match c.b.model.as_str() {
        "compile" => model::compile::run(&c)?,
        "count" => model::count::run(&c)?,
        "count-spans" => model::count_spans::run(&c)?,
        "count-captures" => model::count_captures::run(&c)?,
        "grep" => model::grep::run(&c)?,
        "grep-captures" => model::grep_captures::run(&c)?,
        "regex-redux" => model::regexredux::run(&c)?,
        _ => anyhow::bail!("unsupported benchmark model '{}'", c.b.model),
    };
    if !quiet {
        let mut stdout = std::io::stdout().lock();
        for s in samples.iter() {
            writeln!(stdout, "{},{}", s.duration.as_nanos(), s.count)?;
        }
    }
    Ok(())
}
