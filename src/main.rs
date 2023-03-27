use std::io::Write;

// helpers and other things
mod args;
mod format;
mod util;

// sub-commands
mod build;
mod clean;
mod cmp;
mod diff;
mod haystack;
mod klv;
mod measure;
mod report;

const USAGE: &'static str = "\
A regex barometer tool for running benchmarks and comparing results.

USAGE:
    rebar <command> ...

COMMANDS:
    build     Build regex engines.
    clean     Clean artifacts produced by 'rebar build'.
    cmp       Compare timings across regex engines.
    diff      Compare timings across time for the same regex engine.
    haystack  Print the haystack contents of a benchmark to stdout.
    klv       Print the KLV format of a benchmark.
    measure   Capture timings to CSV by running benchmarks.
    report    Print a Markdown formatted report of benchmark results.

";

fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("warn"),
    )
    .init();
    if let Err(err) = run(&mut lexopt::Parser::from_env()) {
        if std::env::var("RUST_BACKTRACE").map_or(false, |v| v == "1") {
            writeln!(&mut std::io::stderr(), "{:?}", err).unwrap();
        } else {
            writeln!(&mut std::io::stderr(), "{:#}", err).unwrap();
        }
        std::process::exit(1);
    }
    Ok(())
}

fn run(p: &mut lexopt::Parser) -> anyhow::Result<()> {
    let cmd = args::next_as_command(USAGE, p)?;
    match &*cmd {
        "build" => build::run(p),
        "clean" => clean::run(p),
        "cmp" => cmp::run(p),
        "diff" => diff::run(p),
        "haystack" => haystack::run(p),
        "klv" => klv::run(p),
        "measure" => measure::run(p),
        "report" => report::run(p),
        unk => anyhow::bail!("unrecognized command '{}'", unk),
    }
}
