use std::{io::Write, path::PathBuf};

use {anyhow::Context, lexopt::Arg};

use crate::{
    args::{Filter, Usage},
    format::benchmarks::Engines,
    util,
};

const USAGES: &[Usage] =
    &[Usage::BENCH_DIR, Filter::USAGE_ENGINE, Filter::USAGE_ENGINE_NOT];

fn usage() -> String {
    format!(
        "\
This removes the artifacts produced by 'rebar build'. This is useful for cases
where one wants to rebuild one or more regex engines after starting fresh.

USAGE:
    rebar clean [-e <engine> ...]

OPTIONS:
{options}
",
        options = Usage::short(USAGES),
    )
    .trim()
    .to_string()
}

pub fn run(p: &mut lexopt::Parser) -> anyhow::Result<()> {
    let c = Config::parse(p)?;
    let engines =
        Engines::from_file(&c.dir, |e| c.engine_filter.include(&e.name))?;

    let mut out = std::io::stdout().lock();
    for e in engines.list.iter() {
        let prefix = e.name.clone();
        if e.clean.is_empty() {
            continue;
        }
        for cmd in e.clean.iter() {
            let mut proccmd = cmd.command()?;
            writeln!(out, "{}: running: {:?}", prefix, proccmd)?;
            let out = util::output(&mut proccmd)?;
            log::trace!("stdout: {:?}", out);
        }
    }
    Ok(())
}

#[derive(Clone, Debug, Default)]
struct Config {
    dir: PathBuf,
    engine_filter: Filter,
}

impl Config {
    fn parse(p: &mut lexopt::Parser) -> anyhow::Result<Config> {
        let mut c = Config::default();
        c.dir = PathBuf::from("benchmarks");
        while let Some(arg) = p.next()? {
            match arg {
                Arg::Short('h') | Arg::Long("help") => {
                    anyhow::bail!("{}", usage())
                }
                Arg::Short('d') | Arg::Long("dir") => {
                    c.dir = PathBuf::from(p.value().context("-d/--dir")?);
                }
                Arg::Short('e') | Arg::Long("engine") => {
                    c.engine_filter.arg_whitelist(p, "-e/--engine")?;
                }
                Arg::Short('E') | Arg::Long("engine-not") => {
                    c.engine_filter.arg_blacklist(p, "-E/--engine-not")?;
                }
                _ => return Err(arg.unexpected().into()),
            }
        }
        Ok(c)
    }
}
