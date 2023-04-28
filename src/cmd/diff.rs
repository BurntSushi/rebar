use std::{
    collections::btree_map::{BTreeMap, Entry},
    path::{Path, PathBuf},
};

use unicode_width::UnicodeWidthStr;

use crate::{
    args::{self, Color, Filter, Filters, Stat, ThresholdRange, Units, Usage},
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
    Stat::USAGE,
    ThresholdRange::USAGE_MIN,
    ThresholdRange::USAGE_MAX,
    Units::USAGE,
];

fn usage_short() -> String {
    format!(
        "\
Compare benchmarks across time.

USAGE:
    rebar diff [OPTIONS] <csv-path> ...

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
Compare benchmarks across time.

To compare benchmark results between different regex engines for the same
benchmark, use the 'rebar cmp' command.

USAGE:
    rebar diff [OPTIONS] <csv-path> ...

    This command takes one or more file paths to CSV files written by the
    'rebar measure' command. It outputs a comparison for each regex engine
    across time for each benchmark.

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
    let data_names = config.csv_data_names()?;
    let grouped_aggs = config.read_measurement_groups()?;

    let mut wtr = config.color.elastic_stdout();

    // Write column names.
    write!(wtr, "benchmark")?;
    write!(wtr, "\tengine")?;
    for data_name in data_names.iter() {
        write!(wtr, "\t{}", data_name)?;
    }
    writeln!(wtr, "")?;

    // Write underlines beneath each column name to give some separation.
    write_divider(&mut wtr, '-', "benchmark".width())?;
    write!(wtr, "\t")?;
    write_divider(&mut wtr, '-', "engine".width())?;
    for data_name in data_names.iter() {
        write!(wtr, "\t")?;
        write_divider(&mut wtr, '-', data_name.width())?;
    }
    writeln!(wtr, "")?;

    for group in grouped_aggs.iter() {
        if !group.is_within_range(config.stat, config.speedups) {
            continue;
        }
        write!(wtr, "{}", group.name)?;
        write!(wtr, "\t{}", group.engine)?;
        // We write an entry for every data set given, even if this benchmark
        // doesn't appear in every data set. This makes sure everything stays
        // aligned. If an output has too many missing entries, the user can use
        // filters to condense things.
        let best = group.best(config.stat);
        let has_throughput = group.any_throughput();
        for data_name in data_names.iter() {
            write!(wtr, "\t")?;
            match group.measurements_by_data.get(data_name) {
                None => {
                    write!(wtr, "-")?;
                }
                Some(m) => {
                    if best == data_name {
                        let mut spec = termcolor::ColorSpec::new();
                        spec.set_fg(Some(termcolor::Color::Green))
                            .set_bold(true);
                        wtr.set_color(&spec)?;
                    }
                    let ratio = group.ratio(data_name, config.stat);
                    match config.units {
                        Units::Throughput if has_throughput => {
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
                    if best == data_name {
                        wtr.reset()?;
                    }
                }
            }
        }
        writeln!(wtr, "")?;
    }
    wtr.flush()?;
    Ok(())
}

/// The arguments for this 'diff' command parsed from CLI args.
#[derive(Debug, Default)]
struct Config {
    /// File paths to CSV files.
    csv_paths: Vec<PathBuf>,
    /// The benchmark name, model and regex engine filters.
    filters: Filters,
    /// The statistic we want to compare.
    stat: Stat,
    /// The statistical units we want to use in our comparisons.
    units: Units,
    /// The range of speedup ratios to show.
    speedups: ThresholdRange,
    /// The user's color choice. We default to 'Auto'.
    color: Color,
}

impl Config {
    /// Parse 'diff' args from the given CLI parser.
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
                Arg::Short('m') | Arg::Long("model") => {
                    c.filters.model.arg_whitelist(p, "-m/--model")?;
                }
                Arg::Short('M') | Arg::Long("model-not") => {
                    c.filters.model.arg_blacklist(p, "-M/--model-not")?;
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

    /// Reads all aggregate benchmark measurements from all CSV file paths
    /// given, and returns them grouped by the data set. That is, each group
    /// represents all measurements found across the data sets given for a
    /// single (benchmark name, engine name) pair. The filters provided are
    /// applied.
    fn read_measurement_groups(
        &self,
    ) -> anyhow::Result<Vec<MeasurementGroup>> {
        // Our groups are just maps from CSV data name to measurements.
        let mut groups: Vec<BTreeMap<String, Measurement>> = vec![];
        // Map from (benchmark, engine) pair to index in 'groups'. We use the
        // index to find which group to insert each measurement into.
        let mut pair2idx: BTreeMap<(String, String), usize> = BTreeMap::new();
        for csv_path in self.csv_paths.iter() {
            let data_name = csv_data_name(csv_path)?;
            let mut rdr = csv::Reader::from_path(csv_path)?;
            for result in rdr.deserialize() {
                let m: Measurement = result?;
                if let Some(ref err) = m.err {
                    log::warn!(
                        "{}:{}: skipping because of error: {}",
                        m.name,
                        m.engine,
                        err
                    );
                    continue;
                }
                if !self.filters.include(&m) {
                    continue;
                }
                let pair = (m.name.clone(), m.engine.clone());
                let idx = match pair2idx.entry(pair) {
                    Entry::Occupied(e) => *e.get(),
                    Entry::Vacant(e) => {
                        let idx = groups.len();
                        groups.push(BTreeMap::new());
                        *e.insert(idx)
                    }
                };
                groups[idx].insert(data_name.clone(), m);
            }
        }
        Ok(groups.into_iter().map(MeasurementGroup::new).collect())
    }

    /// Returns the "nice" CSV data names from the paths given. These names
    /// are used as the columns in the 'diff' output.
    fn csv_data_names(&self) -> anyhow::Result<Vec<String>> {
        self.csv_paths.iter().map(csv_data_name).collect()
    }
}

/// A group of measurements for a single pair of (benchmark name, engine name).
/// Every measurement in this group represents an aggregate group of statistic
/// from a given CSV input.
#[derive(Debug)]
struct MeasurementGroup {
    /// The benchmark definition's "full name," corresponding to all
    /// measurements in this group. This is mostly just an easy convenience for
    /// accessing the name without having to dig through the map.
    name: String,
    /// Similarly to 'name', this is the regex engine corresponding to all
    /// measurements in this group.
    engine: String,
    /// A map from the data set name to the measurement. Every measurement in
    /// this map must have the same benchmark 'name' and 'engine' name.
    measurements_by_data: BTreeMap<String, Measurement>,
}

impl MeasurementGroup {
    /// Create a new group of aggregates for a single (benchmark name, engine
    /// name) pair. Every aggregate given must have the same 'name' and
    /// 'engine'. Each aggregate is expected to be a measurement from a
    /// distinct CSV input, where the name of the CSV input is the key in the
    /// map given.
    fn new(
        measurements_by_data: BTreeMap<String, Measurement>,
    ) -> MeasurementGroup {
        let mut it = measurements_by_data.values();
        let (name, engine) = {
            let m = it.next().expect("at least one measurement");
            (m.name.clone(), m.engine.clone())
        };
        for m in it {
            assert_eq!(
                name, m.name,
                "expected all measurements to have name {}, \
                 but also found {}",
                name, m.name,
            );
            assert_eq!(
                engine, m.engine,
                "expected all measurements to have engine {}, \
                 but also found {}",
                engine, m.engine,
            );
        }
        MeasurementGroup { name, engine, measurements_by_data }
    }

    /// Return the ratio between the 'this' benchmark and the best benchmark
    /// in the group. The 'this' is the best, then the ratio returned is 1.0.
    /// Thus, the ratio is how many times slower this benchmark is from the
    /// best.
    fn ratio(&self, this: &str, stat: Stat) -> f64 {
        if self.measurements_by_data.len() < 2 {
            // I believe this is a redundant base case.
            return 1.0;
        }
        let this =
            self.measurements_by_data[this].duration(stat).as_secs_f64();
        let best = self.measurements_by_data[self.best(stat)]
            .duration(stat)
            .as_secs_f64();
        this / best
    }

    /// Returns true only when this group contains at least one aggregate
    /// measurement whose speedup ratio falls within the given range.
    ///
    /// The aggregate statistic used to test against the given range is
    /// specified by `stat`.
    fn is_within_range(&self, stat: Stat, range: ThresholdRange) -> bool {
        // We don't filter on the "best" engine below because its speedup ratio
        // is always 1. So if we have a group of size 1, then we don't filter
        // on spedup ratio at all and thus would return false below, which
        // doesn't seem right. So we detect that case and handle it specially
        // here.
        if self.measurements_by_data.len() == 1 {
            return range.contains(1.0);
        }
        let best_data_name = self.best(stat);
        let best = &self.measurements_by_data[best_data_name]
            .duration(stat)
            .as_secs_f64();
        for (data_name, m) in self.measurements_by_data.iter() {
            // The speedup ratio for the best engine is always 1.0, and so it
            // isn't useful to filter on it.
            if data_name == best_data_name {
                continue;
            }
            let this = m.duration(stat).as_secs_f64();
            let ratio = this / best;
            if range.contains(ratio) {
                return true;
            }
        }
        false
    }

    /// Return the data name of the best measurement in this group. The name
    /// returned is guaranteed to exist in this group.
    fn best(&self, stat: Stat) -> &str {
        let mut it = self.measurements_by_data.iter();
        let mut best_data_name = it.next().unwrap().0;
        for (data_name, candidate) in self.measurements_by_data.iter() {
            let best = &self.measurements_by_data[best_data_name];
            if candidate.duration(stat) < best.duration(stat) {
                best_data_name = data_name;
            }
        }
        best_data_name
    }

    /// Returns true if and only if at least one measurement in this group
    /// has throughputs available.
    fn any_throughput(&self) -> bool {
        self.measurements_by_data.values().any(|m| m.aggregate.tputs.is_some())
    }
}

/// Extract a "data set" name from a given CSV file path.
///
/// If there was a problem getting the name (i.e., the file path is "weird" in
/// some way), then an error is returned.
fn csv_data_name<P: AsRef<Path>>(path: P) -> anyhow::Result<String> {
    // This used to just take the file stem from the path's basename, but
    // it's too easy in that case to end up with duplicate names for data
    // sets. So for now, we just take the entire path.
    let path = path.as_ref();
    match path.to_str() {
        Some(name) => Ok(name.to_string()),
        None => anyhow::bail!(
            "{}: path's file name is not valid UTF-8",
            path.display()
        ),
    }
}
