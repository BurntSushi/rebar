use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};

use {anyhow::Context, unicode_width::UnicodeWidthStr};

use crate::{
    args::{self, Color, Filter, Stat, Threshold, Units, Usage},
    format::measurement::Measurement,
    util::{write_divider, ShortHumanDuration},
};

const USAGES: &[Usage] = &[
    Color::USAGE,
    Filter::USAGE_ENGINE,
    Filter::USAGE_ENGINE_NOT,
    Filter::USAGE_BENCH,
    Filter::USAGE_BENCH_NOT,
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
    Threshold::USAGE,
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
    let measurements = config.read_measurements()?;
    let measurements_by_name = MeasurementsByBenchmarkName::new(measurements);
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
                let diff = group.biggest_difference(config.stat);
                if !config.threshold.include(diff) {
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
                let diff = group.biggest_difference(config.stat);
                if !config.threshold.include(diff) {
                    continue;
                }
                write!(wtr, "\t{}", group.name)?;
            }
            writeln!(wtr, "")?;

            // Write underlines beneath each column name to give some
            // separation.
            write_divider(&mut wtr, '-', "engine".width())?;
            for group in measurements_by_name.groups.iter() {
                let diff = group.biggest_difference(config.stat);
                if !config.threshold.include(diff) {
                    continue;
                }
                write!(wtr, "\t")?;
                write_divider(&mut wtr, '-', group.name.width())?;
            }
            writeln!(wtr, "")?;

            for engine in engines.iter() {
                write!(wtr, "{}", engine)?;
                for group in measurements_by_name.groups.iter() {
                    let diff = group.biggest_difference(config.stat);
                    if !config.threshold.include(diff) {
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
    /// A filter to be applied to benchmark "full names."
    bench_filter: Filter,
    /// A filter to be applied to regex engine names.
    engine_filter: Filter,
    /// A filter to be applied to benchmark model name.
    model_filter: Filter,
    /// The statistic we want to compare.
    stat: Stat,
    /// The statistical units we want to use in our comparisons.
    units: Units,
    /// Defaults to 0, and is a percent. When the biggest difference in a row
    /// is less than this threshold, then we skip writing that row.
    threshold: Threshold,
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
                    c.engine_filter.arg_whitelist(p, "-e/--engine")?;
                }
                Arg::Short('E') | Arg::Long("engine-not") => {
                    c.engine_filter.arg_blacklist(p, "-E/--engine-not")?;
                }
                Arg::Short('f') | Arg::Long("filter") => {
                    c.bench_filter.arg_whitelist(p, "-f/--filter")?;
                }
                Arg::Short('F') | Arg::Long("filter-not") => {
                    c.bench_filter.arg_blacklist(p, "-F/--filter-not")?;
                }
                Arg::Short('m') | Arg::Long("model") => {
                    c.model_filter.arg_whitelist(p, "-m/--model")?;
                }
                Arg::Short('M') | Arg::Long("model-not") => {
                    c.model_filter.arg_blacklist(p, "-M/--model-not")?;
                }
                Arg::Long("row") => {
                    c.row = args::parse(p, "--row")?;
                }
                Arg::Short('s') | Arg::Long("statistic") => {
                    c.stat = args::parse(p, "-s/--statistic")?;
                }
                Arg::Short('t') | Arg::Long("threshold") => {
                    c.threshold = args::parse(p, "-t/--threshold")?;
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

    /// Reads all aggregate benchmark measurements from all CSV file paths
    /// given, and returns them as one flattened vector. The filters provided
    /// are applied. If any duplicates are seen (for a given benchmark name and
    /// regex engine pair), then an error is returned.
    fn read_measurements(&self) -> anyhow::Result<Vec<Measurement>> {
        let mut measurements = vec![];
        // A set of (benchmark full name, regex engine name) pairs.
        let mut seen: BTreeSet<(String, String)> = BTreeSet::new();
        for csv_path in self.csv_paths.iter() {
            let mut rdr = csv::Reader::from_path(csv_path)
                .with_context(|| csv_path.display().to_string())?;
            for result in rdr.deserialize() {
                let m: Measurement = result?;
                if let Some(ref err) = m.err {
                    eprintln!(
                        "{}:{}: skipping because of error: {}",
                        m.name, m.engine, err
                    );
                    continue;
                }
                if !self.bench_filter.include(&m.name) {
                    continue;
                }
                if !self.engine_filter.include(&m.engine) {
                    continue;
                }
                if !self.model_filter.include(&m.model) {
                    continue;
                }
                let pair = (m.name.clone(), m.engine.clone());
                anyhow::ensure!(
                    !seen.contains(&pair),
                    "duplicate benchmark with name {} and regex engine {}",
                    m.name,
                    m.engine,
                );
                seen.insert(pair);
                measurements.push(m);
            }
        }
        Ok(measurements)
    }
}

/// A grouping of all measurements into groups where each group corresponds
/// to a single benchmark definition and every measurement in that group
/// corresponds to a distinct regex engine. That is, the groups are rows in the
/// output of this command and the elements in each group are the columns.
#[derive(Debug)]
struct MeasurementsByBenchmarkName {
    groups: Vec<MeasurementGroup>,
}

impl MeasurementsByBenchmarkName {
    /// Group all of the aggregate given.
    fn new(measurements: Vec<Measurement>) -> MeasurementsByBenchmarkName {
        let mut grouped = MeasurementsByBenchmarkName { groups: vec![] };
        // Map from benchmark name to all aggregates with that name in 'aggs'.
        let mut name_to_measurements: BTreeMap<String, Vec<Measurement>> =
            BTreeMap::new();
        for m in measurements {
            name_to_measurements
                .entry(m.name.clone())
                .or_insert(vec![])
                .push(m);
        }
        for (_, measurements) in name_to_measurements {
            grouped.groups.push(MeasurementGroup::new(measurements));
        }
        grouped
    }

    /// Returns a lexicographically sorted list of all regex engine names in
    /// this collection of aggregates. The order is ascending.
    fn engine_names(&self) -> Vec<String> {
        let mut engine_names = BTreeSet::new();
        for group in self.groups.iter() {
            for agg in group.measurements_by_engine.values() {
                engine_names.insert(agg.engine.clone());
            }
        }
        engine_names.into_iter().collect()
    }
}

/// A group of aggregates for a single benchmark name. Every aggregate in this
/// group represents a distinct regex engine for the same benchmark definition.
#[derive(Debug)]
struct MeasurementGroup {
    /// The benchmark definition's name, corresponding to all aggregates
    /// in this group. This is mostly just an easy convenience for accessing
    /// the name without having to dig through the map.
    name: String,
    /// A map from the benchmark's regex engine to the aggregate statistics.
    /// Every aggregate in this map must have the same benchmark 'name'.
    measurements_by_engine: BTreeMap<String, Measurement>,
}

impl MeasurementGroup {
    /// Create a new group of aggregates for a single benchmark name. Every
    /// aggregate given must have the same 'name'. Each aggregate is expected
    /// to be a measurement for a distinct regex engine.
    fn new(measurements: Vec<Measurement>) -> MeasurementGroup {
        let mut measurements_by_engine = BTreeMap::new();
        let name = measurements[0].name.clone();
        for m in measurements {
            assert_eq!(
                name, m.name,
                "expected all aggregates to have name {}, but also found {}",
                name, m.name,
            );
            assert!(
                !measurements_by_engine.contains_key(&m.engine),
                "duplicate regex engine {} for benchmark {}",
                m.engine,
                m.name,
            );
            measurements_by_engine.insert(m.engine.clone(), m);
        }
        MeasurementGroup { name, measurements_by_engine }
    }

    /// Return the biggest difference, percentage wise, between aggregates
    /// in this group. The comparison statistic given is used. If this group
    /// is a singleton, then 0 is returned. (Which makes sense. There is no
    /// difference at all, so specifying any non-zero threshold should exclude
    /// it.)
    fn biggest_difference(&self, stat: Stat) -> f64 {
        if self.measurements_by_engine.len() < 2 {
            // I believe this is a redundant base case.
            return 0.0;
        }
        let best = self.measurements_by_engine[self.best(stat)]
            .duration(stat)
            .as_secs_f64();
        let worst = self.measurements_by_engine[self.worst(stat)]
            .duration(stat)
            .as_secs_f64();
        ((best - worst).abs() / best) * 100.0
    }

    /// Return the ratio between the 'this' engine and the best benchmark in
    /// the group. The 'this' is the best, then the ratio returned is 1.0.
    /// Thus, the ratio is how many times slower this engine is from the best
    /// for this particular benchmark.
    fn ratio(&self, this: &str, stat: Stat) -> f64 {
        if self.measurements_by_engine.len() < 2 {
            // I believe this is a redundant base case.
            return 1.0;
        }
        let this =
            self.measurements_by_engine[this].duration(stat).as_secs_f64();
        let best = self.measurements_by_engine[self.best(stat)]
            .duration(stat)
            .as_secs_f64();
        this / best
    }

    /// Return the engine name of the best measurement in this group. The name
    /// returned is guaranteed to exist in this group.
    fn best(&self, stat: Stat) -> &str {
        let mut it = self.measurements_by_engine.iter();
        let mut best_engine = it.next().unwrap().0;
        for (engine, candidate) in self.measurements_by_engine.iter() {
            let best = &self.measurements_by_engine[best_engine];
            if candidate.duration(stat) < best.duration(stat) {
                best_engine = engine;
            }
        }
        best_engine
    }

    /// Return the engine name of the worst measurement in this group. The name
    /// returned is guaranteed to exist in this group.
    fn worst(&self, stat: Stat) -> &str {
        let mut it = self.measurements_by_engine.iter();
        let mut worst_engine = it.next().unwrap().0;
        for (engine, candidate) in self.measurements_by_engine.iter() {
            let worst = &self.measurements_by_engine[worst_engine];
            if candidate.duration(stat) > worst.duration(stat) {
                worst_engine = engine;
            }
        }
        worst_engine
    }

    /// Returns true if and only if at least one measurement in this group
    /// has throughputs available.
    fn any_throughput(&self) -> bool {
        self.measurements_by_engine
            .values()
            .any(|m| m.aggregate.tputs.is_some())
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
fn write_datum<W: termcolor::WriteColor>(
    config: &Config,
    mut wtr: W,
    group: &MeasurementGroup,
    engine: &str,
) -> anyhow::Result<()> {
    match group.measurements_by_engine.get(engine) {
        None => {
            write!(wtr, "-")?;
        }
        Some(m) => {
            if engine == group.best(config.stat) {
                let mut spec = termcolor::ColorSpec::new();
                spec.set_fg(Some(termcolor::Color::Green)).set_bold(true);
                wtr.set_color(&spec)?;
            }
            let ratio = group.ratio(engine, config.stat);
            match config.units {
                Units::Throughput if group.any_throughput() => {
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
