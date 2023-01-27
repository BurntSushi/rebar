use std::{path::PathBuf, sync::Arc, time::Duration};

use {
    anyhow::Context,
    lexopt::{Arg, ValueExt},
};

use crate::{
    args::{self, Usage},
    format::benchmarks::Benchmarks,
    util::ShortHumanDuration,
};

const USAGES: &[Usage] = &[
    Usage::BENCH_DIR,
    Usage::MAX_ITERS,
    Usage::MAX_WARMUP_ITERS,
    Usage::MAX_TIME,
    Usage::MAX_WARMUP_TIME,
];

fn usage() -> String {
    format!(
        "\
Print the given benchmark in key-length-value (KLV) format.

When using this command, you'll almost always want to set the various --max-*
flags as they all default to 0.

This command is useful when interacting with the benchmark runner programs
directly. Namely, the runner programs accept benchmark definitions in KLV
format on stdin. Normally, that KLV data is generated by the 'rebar measure'
command automatically, but if you're debugging a benchmark runner or just need
to run it directly for whatever reason, then this command is convenient to
generate the necessary KLV data.

See also the 'rebar haystack' command, for cases where you just want the
haystack for a specific benchmark definition.

USAGE:
    rebar klv <benchmark-name>

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
    let mut max_iters = 0;
    let mut max_warmup_iters = 0;
    let mut max_time = Duration::default();
    let mut max_warmup_time = Duration::default();
    while let Some(arg) = p.next()? {
        match arg {
            Arg::Value(name) => {
                if bench_name.is_some() {
                    anyhow::bail!("{}", usage());
                }
                bench_name = Some(name.string()?);
            }
            Arg::Short('d') | Arg::Long("dir") => {
                dir = PathBuf::from(p.value().context("-d/--dir")?);
            }
            Arg::Long("max-iters") => {
                max_iters = args::parse(p, "--max-iters")?;
            }
            Arg::Long("max-warmup-iters") => {
                max_warmup_iters = args::parse(p, "--max-warmup-iters")?;
            }
            Arg::Long("max-time") => {
                let hdur = args::parse::<ShortHumanDuration>(p, "--max-time")?;
                max_time = Duration::from(hdur);
            }
            Arg::Long("max-warmup-time") => {
                let hdur =
                    args::parse::<ShortHumanDuration>(p, "--max-warmup-time")?;
                max_warmup_time = Duration::from(hdur);
            }
            _ => return Err(arg.unexpected().into()),
        }
    }
    let bench_name = match bench_name {
        None => anyhow::bail!("{}", usage()),
        Some(bench_name) => bench_name,
    };
    let def = Benchmarks::find_one(&dir, &bench_name)?;
    let klvbench = klv::Benchmark {
        name: def.name.as_str().to_string(),
        model: def.model.as_str().to_string(),
        regex: klv::Regex {
            patterns: def.regexes.iter().map(|p| p.to_string()).collect(),
            case_insensitive: def.options.case_insensitive,
            unicode: def.options.unicode,
        },
        haystack: Arc::clone(&def.haystack),
        max_iters,
        max_warmup_iters,
        max_time,
        max_warmup_time,
    };
    klvbench
        .write(&mut std::io::stdout())
        .context("failed to write KLV data to stdout")?;
    Ok(())
}
