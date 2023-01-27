use std::{collections::BTreeSet, io::Write, path::PathBuf};

use {
    anyhow::Context,
    lexopt::{Arg, ValueExt},
};

use crate::{args::Usage, format::benchmarks::Engines, util};

const USAGES: &[Usage] = &[Usage::BENCH_DIR];

fn usage() -> String {
    format!(
        "\
This removes the artifacts produced by 'rebar build'. This is useful for cases
where one wants to rebuild one or more regex engines after starting fresh.

USAGE:
    rebar clean [<engine> ...]

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
        Engines::from_file(&c.dir.join("engines.toml"), c.refs().as_ref())?;

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
    engines: Vec<String>,
}

impl Config {
    fn parse(p: &mut lexopt::Parser) -> anyhow::Result<Config> {
        let mut c = Config::default();
        c.dir = PathBuf::from("benchmarks");
        while let Some(arg) = p.next()? {
            match arg {
                Arg::Value(engine) => {
                    c.engines.push(engine.string()?);
                }
                Arg::Short('h') | Arg::Long("help") => {
                    anyhow::bail!("{}", usage())
                }
                Arg::Short('d') | Arg::Long("dir") => {
                    c.dir = PathBuf::from(p.value().context("-d/--dir")?);
                }
                _ => return Err(arg.unexpected().into()),
            }
        }
        Ok(c)
    }

    fn refs(&self) -> Option<BTreeSet<String>> {
        if self.engines.is_empty() {
            return None;
        }
        Some(self.engines.iter().map(|e| e.to_string()).collect())
    }
}
