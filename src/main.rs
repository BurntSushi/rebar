use std::io::Write;

#[macro_use]
mod macros;

mod args;
mod cmd;
mod format;
mod grouped;
mod util;

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
    rank      Print a ranking of regex engines from benchmark results.
    report    Print a Markdown formatted report of benchmark results.
    version   Print the version of rebar and exit.

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
        "build" => cmd::build::run(p),
        "clean" => cmd::clean::run(p),
        "cmp" => cmd::cmp::run(p),
        "diff" => cmd::diff::run(p),
        "haystack" => cmd::haystack::run(p),
        "klv" => cmd::klv::run(p),
        "measure" => cmd::measure::run(p),
        "rank" => cmd::rank::run(p),
        "report" => cmd::report::run(p),
        "version" => cmd::version::run(p),
        unk => anyhow::bail!("unrecognized command '{}'", unk),
    }
}
