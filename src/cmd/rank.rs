use std::{io::Write, path::PathBuf};

use unicode_width::UnicodeWidthStr;

use crate::{
    args::{self, Filter, Filters, Stat, Usage},
    format::measurement::MeasurementReader,
    grouped,
    util::write_divider,
};

const USAGES: &[Usage] = &[
    Filter::USAGE_ENGINE,
    Filter::USAGE_ENGINE_NOT,
    Filter::USAGE_BENCH,
    Filter::USAGE_BENCH_NOT,
    MeasurementReader::USAGE_INTERSECTION,
    Filter::USAGE_MODEL,
    Filter::USAGE_MODEL_NOT,
    Stat::USAGE,
];

fn usage_short() -> String {
    format!(
        "\
Rank the regex engines from the measurements given.

USAGE:
    rebar rank [OPTIONS] <csv-path> ...

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
Rank the regex engines from the measurements given.

The ranking works by computing the speedup ratios for each regex engine for
every benchmark it participates in. A speedup ratio of 1.0 means it was the
fastest engine for a particular benchmark. A speedup ratio of N > 1.0 means
it was N times slower than the fastest regex engine.

Once speedup ratios are computed for each regex engine across all benchmarks,
the speedup ratios are averaged together for each regex engine using the
geometric mean. The geometric mean is used because it is more robust to
outliers than the arithmetic mean.

It is usually desirable to call this command with the --intersection flag,
which limits the geometric mean to only consider speedup ratios in which
all regex engines have measurements.

For example, a pairwise comparison between two regex engines might look like
this:

    rebar rank *.csv --intersection -e '^(rust/regex|hyperscan)$' -M compile

The --intersection flag ensures only benchmarks in which both rust/regex and
hyperscan have measurements are considered. The '-M compile' flag is also given
to filter out measurements from the 'compile' model, since combining search
and compile time measurements into one aggregate is usually not what you want.
You can use '-m compile' to invert it and compute a ranking restricted only to
compile time measurements.

USAGE:
    rebar rank [OPTIONS] <csv-path> ...

    This command takes one or more file paths to CSV files written by the
    'rebar measure' command. It outputs a ranking of all regex engines across
    all measurements given.

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
    let by_name = grouped::ByBenchmarkName::new(&measurements)?;
    let ranking = by_name.ranking(config.stat)?;

    let mut wtr = tabwriter::TabWriter::new(std::io::stdout());
    let columns = &[
        "Engine",
        "Version",
        "Geometric mean of speed ratios",
        "Benchmark count",
    ];
    writeln!(wtr, "{}", columns.join("\t"))?;
    for (i, label) in columns.iter().enumerate() {
        if i > 0 {
            write!(wtr, "\t")?;
        }
        write_divider(&mut wtr, '-', label.width())?;
    }
    write!(wtr, "\n")?;
    for summary in ranking {
        writeln!(
            wtr,
            "{}\t{}\t{:.2}\t{}",
            summary.name, summary.version, summary.geomean, summary.count
        )?;
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
                Arg::Short('s') | Arg::Long("statistic") => {
                    c.stat = args::parse(p, "-s/--statistic")?;
                }
                _ => return Err(arg.unexpected().into()),
            }
        }
        anyhow::ensure!(!c.csv_paths.is_empty(), "no CSV file paths given");
        Ok(c)
    }
}
