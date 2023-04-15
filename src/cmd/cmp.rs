use std::path::PathBuf;

use unicode_width::UnicodeWidthStr;

use crate::{
    args::{self, Color, Filter, Filters, Stat, ThresholdRange, Units, Usage},
    format::measurement::MeasurementReader,
    grouped,
    util::{write_divider, ShortHumanDuration},
};

const USAGES: &[Usage] = &[
    Color::USAGE,
    Filter::USAGE_ENGINE,
    Filter::USAGE_ENGINE_NOT,
    Filter::USAGE_BENCH,
    Filter::USAGE_BENCH_NOT,
    MeasurementReader::USAGE_INTERSECTION,
    Filter::USAGE_MODEL,
    Filter::USAGE_MODEL_NOT,
    Usage::new(
        "--row <type>",
        "One of: benchmark (default) or engine.",
        r#"
This flag sets what the rows are in the table printed. Its value can be either
'benchmark' or 'engine', where 'benchmark' is the default.

By default, the rows are the benchmark and the columns are the regex engine.
But if there are too many engines with very few benchmarks, this format
probably won't work well. In that case, it might make sense to make the rows
the engines instead (which would make the columns the benchmarks).

If you have both a large number of benchmarks and engines, then you'll have to
do some kind of filtering to trim it down.
"#,
    ),
    Stat::USAGE,
    ThresholdRange::USAGE_MIN,
    ThresholdRange::USAGE_MAX,
    Units::USAGE,
];

fn usage_short() -> String {
    format!(
        "\
Compare benchmarks between different regex engines.

USAGE:
    rebar cmp [OPTIONS] <csv-path> ...

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
Compare benchmarks between different regex engines.

To compare benchmark results for the same regex engine across time, use the
'rebar diff' command.

If you find that the table emitted has too many columns to be easily read,
try running with '--row engine' to flip the rows and columns. If that also has
too many columns, you'll want to use one or more of the filter flags to trim
down the results.

USAGE:
    rebar cmp [OPTIONS] <csv-path> ...

    This command takes one or more file paths to CSV files written by the
    'rebar measure' command. It outputs a comparison between the regex engines
    for each benchmark, subject to the filters provided.

TIP:
    use -h for short docs and --help for long docs

OPTIONS:
{options}
",
        options = Usage::long(USAGES),
    )
    .trim()
    .to_string()
}

pub fn run(p: &mut lexopt::Parser) -> anyhow::Result<()> {
    let config = Config::parse(p)?;
    let measurements = MeasurementReader {
        paths: &config.csv_paths,
        filters: &config.filters,
        intersection: config.intersection,
    }
    .read()?;
    let measurements_by_name = grouped::ByBenchmarkName::new(&measurements)?;
    let engines = measurements_by_name.engine_names();
    let mut wtr = config.color.elastic_stdout();

    match config.row {
        RowKind::Benchmark => {
            // Write column names.
            write!(wtr, "benchmark")?;
            for engine in engines.iter() {
                write!(wtr, "\t{}", engine)?;
            }
            writeln!(wtr, "")?;

            // Write underlines beneath each column name to give some
            // separation.
            write_divider(&mut wtr, '-', "benchmark".width())?;
            for engine in engines.iter() {
                write!(wtr, "\t")?;
                write_divider(&mut wtr, '-', engine.width())?;
            }
            writeln!(wtr, "")?;

            for group in measurements_by_name.groups.iter() {
                if !group.is_within_range(config.stat, config.speedups) {
                    continue;
                }
                write!(wtr, "{}", group.name)?;
                // We write an entry for every engine we care about, even if
                // the engine isn't in this group. This makes sure everything
                // stays aligned. If an output has too many missing entries,
                // the user can use filters to condense things.
                for engine in engines.iter() {
                    write!(wtr, "\t")?;
                    write_datum(&config, &mut wtr, &group, &engine)?;
                }
                writeln!(wtr, "")?;
            }
        }
        RowKind::Engine => {
            // Write column names.
            write!(wtr, "engine")?;
            for group in measurements_by_name.groups.iter() {
                if !group.is_within_range(config.stat, config.speedups) {
                    continue;
                }
                write!(wtr, "\t{}", group.name)?;
            }
            writeln!(wtr, "")?;

            // Write underlines beneath each column name to give some
            // separation.
            write_divider(&mut wtr, '-', "engine".width())?;
            for group in measurements_by_name.groups.iter() {
                if !group.is_within_range(config.stat, config.speedups) {
                    continue;
                }
                write!(wtr, "\t")?;
                write_divider(&mut wtr, '-', group.name.width())?;
            }
            writeln!(wtr, "")?;

            for engine in engines.iter() {
                write!(wtr, "{}", engine)?;
                for group in measurements_by_name.groups.iter() {
                    if !group.is_within_range(config.stat, config.speedups) {
                        continue;
                    }
                    write!(wtr, "\t")?;
                    write_datum(&config, &mut wtr, &group, &engine)?;
                }
                writeln!(wtr, "")?;
            }
        }
    }
    wtr.flush()?;
    Ok(())
}

/// The arguments for this 'cmp' command parsed from CLI args.
#[derive(Debug, Default)]
struct Config {
    /// File paths to CSV files.
    csv_paths: Vec<PathBuf>,
    /// The benchmark name, model and regex engine filters.
    filters: Filters,
    /// Whether to only consider benchmarks containing all regex engines.
    intersection: bool,
    /// The statistic we want to compare.
    stat: Stat,
    /// The statistical units we want to use in our comparisons.
    units: Units,
    /// The range of speedup ratios to show.
    speedups: ThresholdRange,
    /// The user's color choice. We default to 'Auto'.
    color: Color,
    /// What the rows of the comparison table should be.
    row: RowKind,
}

impl Config {
    /// Parse 'cmp' args from the given CLI parser.
    fn parse(p: &mut lexopt::Parser) -> anyhow::Result<Config> {
        use lexopt::Arg;

        let mut c = Config::default();
        while let Some(arg) = p.next()? {
            match arg {
                Arg::Value(v) => c.csv_paths.push(PathBuf::from(v)),
                Arg::Short('h') => anyhow::bail!("{}", usage_short()),
                Arg::Long("help") => anyhow::bail!("{}", usage_long()),
                Arg::Long("color") => {
                    c.color = args::parse(p, "-c/--color")?;
                }
                Arg::Short('e') | Arg::Long("engine") => {
                    c.filters.engine.arg_whitelist(p, "-e/--engine")?;
                }
                Arg::Short('E') | Arg::Long("engine-not") => {
                    c.filters.engine.arg_blacklist(p, "-E/--engine-not")?;
                }
                Arg::Short('f') | Arg::Long("filter") => {
                    c.filters.name.arg_whitelist(p, "-f/--filter")?;
                }
                Arg::Short('F') | Arg::Long("filter-not") => {
                    c.filters.name.arg_blacklist(p, "-F/--filter-not")?;
                }
                Arg::Long("intersection") => {
                    c.intersection = true;
                }
                Arg::Short('m') | Arg::Long("model") => {
                    c.filters.model.arg_whitelist(p, "-m/--model")?;
                }
                Arg::Short('M') | Arg::Long("model-not") => {
                    c.filters.model.arg_blacklist(p, "-M/--model-not")?;
                }
                Arg::Long("row") => {
                    c.row = args::parse(p, "--row")?;
                }
                Arg::Short('s') | Arg::Long("statistic") => {
                    c.stat = args::parse(p, "-s/--statistic")?;
                }
                Arg::Short('t') | Arg::Long("threshold-min") => {
                    c.speedups.set_min(args::parse(p, "-t/--threshold-min")?);
                }
                Arg::Short('T') | Arg::Long("threshold-max") => {
                    c.speedups.set_max(args::parse(p, "-T/--threshold-max")?);
                }
                Arg::Short('u') | Arg::Long("units") => {
                    c.units = args::parse(p, "-u/--units")?;
                }
                _ => return Err(arg.unexpected().into()),
            }
        }
        anyhow::ensure!(!c.csv_paths.is_empty(), "no CSV file paths given");
        Ok(c)
    }
}

/// The entity to use for the rows in the comparison table printed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RowKind {
    Benchmark,
    Engine,
}

impl Default for RowKind {
    fn default() -> RowKind {
        RowKind::Benchmark
    }
}

impl std::str::FromStr for RowKind {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> anyhow::Result<RowKind> {
        Ok(match s {
            "benchmark" => RowKind::Benchmark,
            "engine" => RowKind::Engine,
            unknown => anyhow::bail!("unrecognized row kind '{}'", unknown),
        })
    }
}

/// Writes a single aggregate statistic for the given engine from the given
/// group of measurements.
fn write_datum<T, W: termcolor::WriteColor>(
    config: &Config,
    mut wtr: W,
    group: &grouped::ByBenchmarkNameGroup<T>,
    engine: &str,
) -> anyhow::Result<()> {
    match group.by_engine.get(engine) {
        None => {
            write!(wtr, "-")?;
        }
        Some(m) => {
            if engine == group.best(config.stat) {
                let mut spec = termcolor::ColorSpec::new();
                spec.set_fg(Some(termcolor::Color::Green)).set_bold(true);
                wtr.set_color(&spec)?;
            }
            let ratio = group.ratio(engine, config.stat).unwrap();
            match config.units {
                Units::Throughput if m.aggregate.tputs.is_some() => {
                    if let Some(tput) = m.throughput(config.stat) {
                        write!(wtr, "{} ({:.2}x)", tput, ratio)?;
                    } else {
                        write!(wtr, "NO-THROUGHPUT")?;
                    }
                }
                _ => {
                    let d = m.duration(config.stat);
                    let humand = ShortHumanDuration::from(d);
                    write!(wtr, "{} ({:.2}x)", humand, ratio)?;
                }
            }
            if engine == group.best(config.stat) {
                wtr.reset()?;
            }
        }
    }
    Ok(())
}
