use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    time::Duration,
};

use anyhow::Context;

use crate::{
    args::{Filters, Stat, Usage},
    util::{ShortHumanDuration, Throughput},
};

/// A simple loader for reading and deserializing measurements, with filter
/// logic, from zero or more CSV files.
///
/// The lifetime `'a` is the shortest lifetime of the slice of file paths and
/// the filters.
#[derive(Clone, Debug)]
pub struct MeasurementReader<'a> {
    /// The CSV file paths to read from.
    pub paths: &'a [PathBuf],
    /// The filters to apply to each measurement. Any measurement that doesn't
    /// pass this filter is dropped.
    pub filters: &'a Filters,
    /// Whether to only retain measurements for which there are measurements
    /// for every regex engine.
    pub intersection: bool,
}

impl<'p> MeasurementReader<'p> {
    pub const USAGE_INTERSECTION: Usage = Usage::new(
        "--intersection",
        "Only consider benchmarks for which all engines participate.",
        r#"
When this flag is set, benchmarks that do not include all regex engines will
be excluded from the report. The set of all regex engines is determined by
unioning the sets of all measurements given to this command.

This is applied after the various filters. So for example, one could pass a
filter like `-e '^(rust/regex|hyperscan)$'` to limit a comparison to only those
two regex engines. That is, only benchmarks containing measurements for both
'rust/regex' and 'hyperscan' will be included.
"#,
    );

    /// Attempts to load measurements from the given loader configuration. If
    /// there was a problem reading the files or if there are any duplicate
    /// measurements.
    pub fn read(self) -> anyhow::Result<Vec<Measurement>> {
        let mut measurements = vec![];
        // A map from benchmark full name to the set of regex engines
        // for which we have measurements. We use this to detect duplicate
        // measurements, and it's also how we implement the 'intersection'
        // filtering.
        let mut name_to_engines: BTreeMap<String, BTreeSet<String>> =
            BTreeMap::new();
        for path in self.paths.iter() {
            let mut rdr = csv::Reader::from_path(path)
                .with_context(|| path.display().to_string())?;
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
                let is_new = name_to_engines
                    .entry(m.name.clone())
                    .or_insert_with(|| BTreeSet::new())
                    .insert(m.engine.clone());
                anyhow::ensure!(
                    is_new,
                    "duplicate measurement with name '{}' \
                     and regex engine '{}'",
                    m.name,
                    m.engine,
                );
                measurements.push(m);
            }
        }
        if self.intersection {
            let engines_len = name_to_engines
                .values()
                .map(|set| set.len())
                .max()
                .unwrap_or(0);
            measurements
                .retain(|m| name_to_engines[&m.name].len() == engines_len);
        }
        Ok(measurements)
    }
}

/// The in-memory representation of a single set of results for one benchmark
/// execution. It does not include all samples taken (those are thrown away and
/// not recorded anywhere), but does include aggregate statistics about the
/// samples.
///
/// Note that when 'err' is set, most other fields are set to their
/// empty/default values.
#[derive(Clone, Debug, Default, serde::Deserialize, serde::Serialize)]
#[serde(from = "WireMeasurement", into = "WireMeasurement")]
pub struct Measurement {
    pub name: String,
    pub model: String,
    pub rebar_version: String,
    pub engine: String,
    pub engine_version: String,
    pub err: Option<String>,
    pub iters: u64,
    pub total: Duration,
    pub aggregate: Aggregate,
}

/// The aggregate statistics computed from samples taken from a benchmark.
///
/// This includes aggregate timings and throughputs, but only the latter when
/// the benchmark includes a haystack length.
#[derive(Clone, Debug, Default)]
pub struct Aggregate {
    pub times: AggregateTimes,
    pub tputs: Option<AggregateThroughputs>,
}

/// The aggregate timings.
#[derive(Clone, Debug, Default)]
pub struct AggregateTimes {
    pub median: Duration,
    pub mad: Duration,
    pub mean: Duration,
    pub stddev: Duration,
    pub min: Duration,
    pub max: Duration,
}

/// The aggregate throughputs. The `len` field is guaranteed to be non-zero.
#[derive(Clone, Debug, Default)]
pub struct AggregateThroughputs {
    pub len: u64,
    pub median: Throughput,
    pub mad: Throughput,
    pub mean: Throughput,
    pub stddev: Throughput,
    pub min: Throughput,
    pub max: Throughput,
}

impl Measurement {
    /// Get the corresponding throughput statistic from this aggregate.
    ///
    /// If this measurement doesn't have any throughputs (i.e., its haystack
    /// length is missing or zero), then this returns `None` regardless of the
    /// value of `stat`.
    pub fn throughput(&self, stat: Stat) -> Option<Throughput> {
        let tputs = self.aggregate.tputs.as_ref()?;
        Some(match stat {
            Stat::Median => tputs.median,
            Stat::Mad => tputs.mad,
            Stat::Mean => tputs.mean,
            Stat::Stddev => tputs.stddev,
            Stat::Min => tputs.min,
            Stat::Max => tputs.max,
        })
    }

    /// Get the corresponding duration statistic from this aggregate.
    pub fn duration(&self, stat: Stat) -> Duration {
        let times = &self.aggregate.times;
        match stat {
            Stat::Median => times.median,
            Stat::Mad => times.mad,
            Stat::Mean => times.mean,
            Stat::Stddev => times.stddev,
            Stat::Min => times.min,
            Stat::Max => times.max,
        }
    }
}

impl Aggregate {
    /// Creates a new set of aggregate statistics.
    ///
    /// If a non-zero haystack length is provided, then the aggregate returned
    /// includes throughputs.
    pub fn new(times: AggregateTimes, haystack_len: Option<u64>) -> Aggregate {
        let tputs = haystack_len.and_then(|len| {
            // We treat an explicit length of 0 and a totally missing value as
            // the same. In practice, there is no difference. We can't get a
            // meaningful throughput with a zero length haystack.
            if len == 0 {
                return None;
            }
            Some(AggregateThroughputs {
                len,
                median: Throughput::new(len, times.median),
                mad: Throughput::new(len, times.mad),
                mean: Throughput::new(len, times.mean),
                stddev: Throughput::new(len, times.stddev),
                min: Throughput::new(len, times.min),
                max: Throughput::new(len, times.max),
            })
        });
        Aggregate { times, tputs }
    }
}

/// The wire Serde type corresponding to a single CSV record in the output of
/// 'rebar measure'.
///
/// The main difference between the wire format and the in-memory format is
/// that the wire format only includes the absolute aggregate timings, where
/// as the in-memory format includes both the aggregate timings and aggregate
/// throughputs. (Throughputs are completely determined by the combination
/// of timings and a haystack length, but only when the haystack length is
/// present.)
#[derive(Clone, Debug, Default, serde::Deserialize, serde::Serialize)]
struct WireMeasurement {
    name: String,
    model: String,
    rebar_version: String,
    engine: String,
    engine_version: String,
    err: Option<String>,
    haystack_len: Option<u64>,
    iters: u64,
    #[serde(serialize_with = "ShortHumanDuration::serialize_with")]
    #[serde(deserialize_with = "ShortHumanDuration::deserialize_with")]
    total: Duration,
    #[serde(serialize_with = "ShortHumanDuration::serialize_with")]
    #[serde(deserialize_with = "ShortHumanDuration::deserialize_with")]
    median: Duration,
    #[serde(serialize_with = "ShortHumanDuration::serialize_with")]
    #[serde(deserialize_with = "ShortHumanDuration::deserialize_with")]
    mad: Duration,
    #[serde(serialize_with = "ShortHumanDuration::serialize_with")]
    #[serde(deserialize_with = "ShortHumanDuration::deserialize_with")]
    mean: Duration,
    #[serde(serialize_with = "ShortHumanDuration::serialize_with")]
    #[serde(deserialize_with = "ShortHumanDuration::deserialize_with")]
    stddev: Duration,
    #[serde(serialize_with = "ShortHumanDuration::serialize_with")]
    #[serde(deserialize_with = "ShortHumanDuration::deserialize_with")]
    min: Duration,
    #[serde(serialize_with = "ShortHumanDuration::serialize_with")]
    #[serde(deserialize_with = "ShortHumanDuration::deserialize_with")]
    max: Duration,
}

impl From<WireMeasurement> for Measurement {
    fn from(w: WireMeasurement) -> Measurement {
        let times = AggregateTimes {
            median: w.median,
            mad: w.mad,
            mean: w.mean,
            stddev: w.stddev,
            min: w.min,
            max: w.max,
        };
        let aggregate = Aggregate::new(times, w.haystack_len);
        Measurement {
            name: w.name,
            model: w.model,
            rebar_version: w.rebar_version,
            engine: w.engine,
            engine_version: w.engine_version,
            err: w.err,
            iters: w.iters,
            total: w.total,
            aggregate,
        }
    }
}

impl From<Measurement> for WireMeasurement {
    fn from(m: Measurement) -> WireMeasurement {
        WireMeasurement {
            name: m.name,
            model: m.model,
            rebar_version: m.rebar_version,
            engine: m.engine,
            engine_version: m.engine_version,
            haystack_len: m.aggregate.tputs.map(|x| x.len),
            err: m.err,
            iters: m.iters,
            total: m.total,
            median: m.aggregate.times.median,
            mad: m.aggregate.times.mad,
            mean: m.aggregate.times.mean,
            stddev: m.aggregate.times.stddev,
            min: m.aggregate.times.min,
            max: m.aggregate.times.max,
        }
    }
}
