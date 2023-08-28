use std::{io::Write, path::PathBuf};

use {anyhow::Context, bstr::ByteSlice, lexopt::Arg};

use crate::{
    args::{self, Color, Filter, Usage},
    format::benchmarks::{Engine, Engines},
    util,
};

const USAGES: &[Usage] = &[
    Usage::BENCH_DIR,
    Color::USAGE,
    Filter::USAGE_ENGINE,
    Filter::USAGE_ENGINE_NOT,
];

fn usage_short() -> String {
    format!(
        "\
This command builds runner programs that expose regex engines to rebar.

USAGE:
    rebar build [-e <engine> ...]

TIP:
    use -h for short docs and --help for long docs

OPTIONS:
{options}
",
        options = Usage::short(USAGES),
    )
    .trim()
    .to_string()
}

fn usage_long() -> String {
    format!(
        "\
This command builds runner programs that expose regex engines to rebar.

One a runner program is built, its version is queried and printed as part of
the output of this program. The version number is meant to act as a receipt
that the runner program has been successfully built and can be executed in the
current environment.

If building a runner program fails, then a short error message is printed.
Building then continues with the other runner programs. Rebar in general does
*not* need to have every runner program build successfully in order to run.
If a runner program fails to build, then collecting measurements will show
an error. But those can be squashed with the -i/--ignore-missing-engines flag.

If a regex engine fails to build, then running this command again with the
environment variable RUSTLOG set to 'debug' will show more output from the
failing commands.

Use the -e/--engine flag to build a subset of engines.

USAGE:
    rebar build [-e <engine> ...]

OPTIONS:
{options}
",
        options = Usage::long(USAGES),
    )
    .trim()
    .to_string()
}

pub fn run(p: &mut lexopt::Parser) -> anyhow::Result<()> {
    let c = Config::parse(p)?;
    let engines =
        Engines::from_file(&c.dir, |e| c.engine_filter.include(&e.name))?;

    let mut printed_note = false;
    let mut printed_dep_note = false;
    let mut out = std::io::stdout().lock();
    let mut stderr = c.color.stderr();
    'ENGINES: for e in engines.list.iter() {
        for dep in e.dependency.iter() {
            let mut stdcmd = dep.run.command()?;
            let out = match util::output(&mut stdcmd) {
                Ok(out) => out,
                Err(err) => {
                    util::colorize_label(&mut stderr, |w| {
                        write!(w, "{}: ", e.name)
                    })?;
                    util::colorize_error(&mut stderr, |w| {
                        write!(w, "dependency command failed: ")
                    })?;
                    writeln!(stderr, "{}", err)?;
                    print_dep_note(&mut stderr, e, &mut printed_dep_note)?;
                    print_note(&mut stderr, e, &mut printed_note)?;
                    continue 'ENGINES;
                }
            };
            let outstr = match out.to_str() {
                Ok(outstr) => outstr,
                Err(err) => {
                    util::colorize_label(&mut stderr, |w| {
                        write!(w, "{}: ", e.name)
                    })?;
                    util::colorize_error(&mut stderr, |w| {
                        write!(
                            w,
                            "dependency command output is not UTF-8: {}",
                            err,
                        )
                    })?;
                    print_dep_note(&mut stderr, e, &mut printed_dep_note)?;
                    print_note(&mut stderr, e, &mut printed_note)?;
                    continue 'ENGINES;
                }
            };
            if let Some(ref re) = dep.regex {
                if !re.is_match(outstr) {
                    util::colorize_label(&mut stderr, |w| {
                        write!(w, "{}: ", e.name)
                    })?;
                    util::colorize_error(&mut stderr, |w| {
                        write!(
                            w,
                            "dependency command did not \
                             print expected output: ",
                        )
                    })?;
                    writeln!(
                        stderr,
                        "could not find match for {:?} in output of {:?}",
                        re.as_str(),
                        stdcmd,
                    )?;
                    print_dep_note(&mut stderr, e, &mut printed_dep_note)?;
                    print_note(&mut stderr, e, &mut printed_note)?;
                    if out.trim_with(|c| c.is_whitespace()).is_empty() {
                        log::debug!(
                            "output for dependency command {:?}: <EMPTY>",
                            stdcmd,
                        );
                    } else {
                        log::debug!(
                            "output for dependency command {:?}: {}",
                            stdcmd,
                            out,
                        );
                    }
                    continue 'ENGINES;
                }
            }
        }
        if e.build.is_empty() {
            if e.is_missing_version() {
                util::colorize_label(&mut stderr, |w| {
                    write!(w, "{}: ", e.name)
                })?;
                util::colorize_error(&mut stderr, |w| {
                    writeln!(w, "no build steps, but version is missing")
                })?;
                print_note(&mut stderr, e, &mut printed_note)?;
            } else {
                util::colorize_label(&mut stderr, |w| {
                    write!(w, "{}: ", e.name)
                })?;
                writeln!(out, "nothing to do")?;
            }
            continue;
        }
        for cmd in e.build.iter() {
            let mut stdcmd = cmd.command()?;
            util::colorize_label(&mut stderr, |w| write!(w, "{}: ", e.name))?;
            writeln!(out, "running: {:?}", stdcmd)?;
            let out = match util::output(&mut stdcmd) {
                Ok(out) => out,
                Err(err) => {
                    util::colorize_label(&mut stderr, |w| {
                        write!(w, "{}: ", e.name)
                    })?;
                    util::colorize_error(&mut stderr, |w| {
                        write!(w, "build failed: ")
                    })?;
                    writeln!(stderr, "{}", err)?;
                    print_note(&mut stderr, e, &mut printed_note)?;
                    continue 'ENGINES;
                }
            };
            log::trace!("stdout: {:?}", out);
        }
        let version = e.version_config.get()?;
        util::colorize_label(&mut stderr, |w| write!(w, "{}: ", e.name))?;
        writeln!(out, "build complete for version {}", version)?;
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
                Arg::Short('h') => anyhow::bail!("{}", usage_short()),
                Arg::Long("help") => anyhow::bail!("{}", usage_long()),
                Arg::Long("color") => {
                    c.color = args::parse(p, "-c/--color")?;
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

fn print_dep_note<W: termcolor::WriteColor>(
    mut wtr: W,
    engine: &Engine,
    printed: &mut bool,
) -> anyhow::Result<()> {
    if *printed {
        return Ok(());
    }
    util::colorize_note(&mut wtr, |w| write!(w, "note: "))?;
    writeln!(
        wtr,
        "a dependency that is required to build '{}' could \
         not be found, either because it isn't installed \
         or because it didn't behave as expected",
        engine.name,
    )?;
    *printed = true;
    Ok(())
}

fn print_note<W: termcolor::WriteColor>(
    mut wtr: W,
    engine: &Engine,
    printed: &mut bool,
) -> anyhow::Result<()> {
    if *printed {
        return Ok(());
    }
    util::colorize_note(&mut wtr, |w| write!(w, "note: "))?;
    writeln!(
        wtr,
        "run `RUST_LOG=debug rebar build -e '^{}$'` to see more details",
        engine.name,
    )?;
    *printed = true;
    Ok(())
}
