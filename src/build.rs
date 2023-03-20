use std::{io::Write, path::PathBuf};

use {anyhow::Context, lexopt::Arg};

use crate::{
    args::{self, Color, Filter, Usage},
    format::benchmarks::Engines,
    util,
};

const USAGES: &[Usage] =
    &[Usage::BENCH_DIR, Color::USAGE, Filter::USAGE_ENGINE];

fn usage() -> String {
    format!(
        "\
This builds runner programs that expose regex engines to rebar.

One a runner program is built, its version is queried and printed as part of
the output of this program. The version number is meant to act as a receipt
that the runner program has been successfully built and can be executed in the
current environment.

If building a runner program fails, then a short error message is printed.
Building then continues with the other runner programs. Rebar in general does
*not* need to have every runner program build successfully in order to run.
If a runner program fails to build, then collecting measurements will show
an error. But those can be squashed with the -i/--ignore-broken-engines flag.

USAGE:
    rebar build [<engine> ...]

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
    let engines = Engines::from_file(&c.dir.join("engines.toml"), None)?;

    let mut printed_note = false;
    let mut out = std::io::stdout().lock();
    let mut stderr = c.color.stderr();
    'ENGINES: for e in engines.list.iter() {
        if !c.engine_filter.include(&e.name) {
            continue;
        }
        let prefix = e.name.clone();
        if e.build.is_empty() {
            if e.is_missing_version() {
                writeln!(
                    out,
                    "{}: no build steps, but version is missing",
                    prefix
                )?;
            } else {
                writeln!(out, "{}: nothing to do", prefix)?;
            }
            continue;
        }
        for cmd in e.build.iter() {
            let mut proccmd = cmd.command()?;
            writeln!(out, "{}: running: {:?}", prefix, proccmd)?;
            let out = match util::output(&mut proccmd) {
                Ok(out) => out,
                Err(err) => {
                    let mut spec = termcolor::ColorSpec::new();
                    spec.set_fg(Some(termcolor::Color::Red)).set_bold(true);
                    stderr.set_color(&spec)?;
                    write!(stderr, "{}: build failed: ", prefix)?;
                    stderr.reset()?;
                    writeln!(stderr, "{}", err)?;
                    if !printed_note {
                        let mut spec = termcolor::ColorSpec::new();
                        spec.set_fg(Some(termcolor::Color::Blue))
                            .set_bold(true);
                        stderr.set_color(&spec)?;
                        write!(stderr, "note: ")?;
                        stderr.reset()?;
                        writeln!(
                            stderr,
                            "run `RUST_LOG=debug rebar build -e '^{}$'` \
                             to see more details",
                            e.name,
                        )?;
                        printed_note = true;
                    }
                    continue 'ENGINES;
                }
            };
            log::trace!("stdout: {:?}", out);
        }
        let version = e.version_config.get()?;
        writeln!(out, "{}: build complete for version {}", prefix, version)?;
    }
    Ok(())
}

#[derive(Clone, Debug, Default)]
struct Config {
    dir: PathBuf,
    engine_filter: Filter,
    color: Color,
}

impl Config {
    fn parse(p: &mut lexopt::Parser) -> anyhow::Result<Config> {
        let mut c = Config::default();
        c.dir = PathBuf::from("benchmarks");
        while let Some(arg) = p.next()? {
            match arg {
                Arg::Long("color") => {
                    c.color = args::parse(p, "-c/--color")?;
                }
                Arg::Short('h') | Arg::Long("help") => {
                    anyhow::bail!("{}", usage())
                }
                Arg::Short('d') | Arg::Long("dir") => {
                    c.dir = PathBuf::from(p.value().context("-d/--dir")?);
                }
                Arg::Short('e') | Arg::Long("engine") => {
                    c.engine_filter.add(args::parse(p, "-e/--engine")?);
                }
                _ => return Err(arg.unexpected().into()),
            }
        }
        Ok(c)
    }
}
