use std::{io::Write, path::PathBuf};

use {
    anyhow::Context,
    lexopt::{Arg, ValueExt},
};

use crate::{
    args::{self, Usage},
    format::benchmarks::Benchmarks,
};

const USAGES: &[Usage] = &[
    Usage::BENCH_DIR,
    Usage::new(
        "-r, --repeat <number>",
        "Repeats the haystack this many times.",
        r#"
Repeats the haystack this many times.

This is useful for doing ad hoc benchmarking. Namely, sometimes it can be
useful to observe the impact of the size of the haystack on the execution time
of the benchmark.
"#,
    ),
];

fn usage() -> String {
    format!(
        "\
Print the contents of a benchmark's haystack to stdout.

While most haystacks are easy to find as files in 'benchmarks/haystacks',
benchmark definitions can actually modify the haystack read from the file
system such that the actual haystack used in the benchmark is different than
what's in the file. This might involve trimming the haystack or appending a
string to the end, for example. This permits the benchmark definitions to reuse
haystacks with small tweaks, in order to avoid bloating the repository size.

The haystack printed to stdout will match precisely the haystack used for the
corresponding benchmark.

If no benchmarks match the given name exactly, then this command reports an
error.

USAGE:
    rebar haystack <benchmark-name>

OPTIONS:
{options}
",
        options = Usage::short(USAGES),
    )
    .trim()
    .to_string()
}

pub fn run(p: &mut lexopt::Parser) -> anyhow::Result<()> {
    let mut bench_name = None;
    let mut dir = PathBuf::from("benchmarks");
    let mut repeat = 1;
    while let Some(arg) = p.next()? {
        match arg {
            Arg::Value(name) => {
                if bench_name.is_some() {
                    anyhow::bail!("{}", usage());
                }
                bench_name = Some(name.string()?);
            }
            Arg::Short('h') | Arg::Long("help") => {
                anyhow::bail!("{}", usage())
            }
            Arg::Short('d') | Arg::Long("dir") => {
                dir = PathBuf::from(p.value().context("-d/--dir")?);
            }
            Arg::Short('r') | Arg::Long("repeat") => {
                repeat = args::parse(p, "-r/--repeat")?;
            }
            _ => return Err(arg.unexpected().into()),
        }
    }
    let bench_name = match bench_name {
        None => anyhow::bail!("{}", usage()),
        Some(bench_name) => bench_name,
    };
    let def = Benchmarks::find_one(&dir, &bench_name)?;
    for _ in 0..repeat {
        if let Err(err) = std::io::stdout().write_all(&def.haystack) {
            if err.kind() == std::io::ErrorKind::BrokenPipe {
                break;
            }
            return Err(anyhow::Error::from(err)
                .context("failed to write haystack to stdout"));
        }
    }
    Ok(())
}
