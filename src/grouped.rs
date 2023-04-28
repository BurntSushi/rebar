/*!
This module provides some types and routines for grouping benchmarks.
Principally, we provide a way to take a sequence of measurements and group them
by benchmark name, where each group is itself just a map from engine name to
the measurement itself.

This group can then be further and optionally tagged with the actual
corresponding benchmark definition from a TOML file.

This sort of grouping is particularly useful because it reflects the
predominant mode of comparison in rebar. That is, we often was to compare the
relative performance of regex engines on the same benchmark. This grouping is
also the place at which speedup ratios can be computed. When you have many of
these grouping, the speedup ratios can be averaged together via a geometric
mean to get a relative summary statistic comparing regex engines across many
benchmarks.
*/

use std::collections::{BTreeMap, BTreeSet};

use crate::{
    args::{Stat, ThresholdRange},
    format::{benchmarks::Definition, measurement::Measurement},
};

/// Groups measurements by benchmark name.
///
/// Each group contains measurements with the same benchmark name. Each group
/// further provides access to each of the measurements by engine name.
/// Construction of this grouping mechanism ensures that none of the groups
/// contain at most one measurement for each engine name.
///
/// The type parameter corresponds to any additional data attached to
/// each group. It is usually either `()` (for no additional data) or a
/// `Definition`, corresponding to the benchmark definition for the group of
/// measurements.
#[derive(Clone, Debug)]
pub struct ByBenchmarkName<T> {
    pub groups: Vec<ByBenchmarkNameGroup<T>>,
}

impl ByBenchmarkName<()> {
    /// Create a new grouping of the given measurements, such that each
    /// measurement is put into a group with all other measurements sharing the
    /// same benchmark name.
    ///
    /// This returns an error if there is more than one measurement with the
    /// same benchmark name and engine name. This also returns an error if any
    /// two measurements with the same engine have a different version.
    pub fn new(
        measurements: &[Measurement],
    ) -> anyhow::Result<ByBenchmarkName<()>> {
        use std::collections::btree_map::Entry;

        // This is a BTreeMap<BenchmarkName, BTreeMap<EngineName, Measurement>>
        let mut map = BTreeMap::new();
        // We use this map to ensure the version is the same for all
        // measurements with the same engine name.
        let mut versions = BTreeMap::new();
        // The sequence of benchmark names, seen in order. This preserves the
        // order of the measurements, such that the groups returned are in the
        // same order as the first appearance of each benchmark in the list of
        // measurements given.
        let mut order = vec![];
        for m in measurements.iter() {
            if !map.contains_key(&m.name) {
                order.push(&m.name);
                map.insert(m.name.clone(), BTreeMap::default());
            }
            match versions.entry(m.engine.clone()) {
                Entry::Vacant(e) => {
                    e.insert(m.engine_version.clone());
                }
                Entry::Occupied(e) => {
                    anyhow::ensure!(
                        e.get() == &m.engine_version,
                        "found mismatching versions '{}' and '{}' for \
                         engine '{}'",
                        m.engine_version,
                        e.get(),
                        m.engine,
                    );
                }
            }
            let result = map
                .get_mut(&m.name)
                .unwrap()
                .insert(m.engine.clone(), m.clone());
            anyhow::ensure!(
                result.is_none(),
                "found measurement for benchmark '{}' with duplicative \
                 engine name '{}'",
                m.name,
                m.engine,
            );
        }
        let mut groups = vec![];
        for name in order {
            let by_engine = map.remove(name).unwrap();
            groups.push(ByBenchmarkNameGroup::new(by_engine));
        }
        Ok(ByBenchmarkName { groups })
    }
}

impl<T> ByBenchmarkName<T> {
    /// Associates each group of benchmarks with the corresponding definition
    /// in the benchmark definitions given.
    ///
    /// This returns an error if there are any duplicate definitions.
    ///
    /// If there are any groups of measurements that do not have a
    /// corresponding definition, then a WARN-level log message is emitted and
    /// are subsequently dropped.
    pub fn associate(
        self,
        defs: Vec<Definition>,
    ) -> anyhow::Result<ByBenchmarkName<Definition>> {
        // Re-organize the groups into a map by benchmark name so that we
        // can find each group according to the definition.
        let mut oldgroups: BTreeMap<String, ByBenchmarkNameGroup<T>> =
            self.groups.into_iter().map(|g| (g.name.clone(), g)).collect();

        // Now rebuild the groups according to the order of `defs`. This way,
        // the order of the groups matches the order of the definitions.
        let mut groups = vec![];
        for def in defs {
            let oldgroup = match oldgroups.remove(&def.name.to_string()) {
                Some(oldgroup) => oldgroup,
                None => {
                    // We are pretty quiet about any definitions without any
                    // measurements, since measurements are usually the focal
                    // point. The definitions are "just" the meta data.
                    log::debug!(
                        "found benchmark definition '{}' without any \
                         associated measurements, this is okay, skipping",
                        def.name,
                    );
                    continue;
                }
            };
            // We also need to check that the definition actually has an
            // engine entry for each measurement in this group.
            let engines: BTreeSet<String> =
                def.engines.iter().map(|e| e.name.clone()).collect();
            let mut by_engine = BTreeMap::new();
            for (engine_name, m) in oldgroup.by_engine {
                if !engines.contains(&engine_name) {
                    log::warn!(
                        "could not find engine '{}' in benchmark \
                         definition for '{}', therefore rebar is \
                         dropping the measurement for this engine",
                        engine_name,
                        def.name,
                    );
                    continue;
                }
                by_engine.insert(engine_name, m);
            }
            groups.push(ByBenchmarkNameGroup {
                name: oldgroup.name,
                by_engine,
                data: def,
            });
        }
        // Any oldgroups left in our map are those that did not have a
        // corresponding definition. We drop them completely, but we warn
        // about it since measurements are usually the focal point. We want
        // to be loud if we're dropping such data.
        for (_, oldgroup) in oldgroups {
            log::warn!(
                "could not find benchmark '{}' in set of \
                 definitions, therefore rebar is dropping all \
                 measurements for that benchmark",
                oldgroup.name,
            );
        }
        Ok(ByBenchmarkName { groups })
    }

    /// Partitions this grouping according to the predicate given. The first
    /// element of the tuple contains all groups for which the given predicate
    /// returns true and the second element of the tuple contains all groups
    /// for which the given predicate returns false.
    ///
    /// It is possible for one element of the tuple to contain zero groups.
    pub fn partition(
        self,
        mut predicate: impl FnMut(&ByBenchmarkNameGroup<T>) -> bool,
    ) -> (ByBenchmarkName<T>, ByBenchmarkName<T>) {
        let (mut true_groups, mut false_groups) = (vec![], vec![]);
        for group in self.groups {
            if predicate(&group) {
                true_groups.push(group);
            } else {
                false_groups.push(group);
            }
        }
        let true_grouping = ByBenchmarkName { groups: true_groups };
        let false_grouping = ByBenchmarkName { groups: false_groups };
        (true_grouping, false_grouping)
    }

    /// Returns all of the engines (name, version and geometric mean of speed
    /// ratios) from the *measurements*.
    ///
    /// Technically the same information is available via the benchmark
    /// definitions, but that reflects the *current* version at the time of
    /// report generation and not the version at the time the measurements
    /// were collected. It's likely that reports are generated right after
    /// collecting measurements, but in case it isn't, the version information
    /// in the report could wind up being quite misleading if we don't take
    /// it directly from measurements.
    ///
    /// THe vector returned is sorted by geometric mean of the speedup ratios
    /// across all participating benchmarks in ascending order.
    pub fn ranking(&self, stat: Stat) -> anyhow::Result<Vec<EngineSummary>> {
        /// This is like EngineSummary, but contains all of the speedup ratios.
        /// The speedup ratios are converted to a geometric mean at the end.
        #[derive(Debug)]
        struct SummaryWithData {
            name: String,
            version: String,
            ratios: Vec<f64>,
        }

        let mut map: BTreeMap<String, SummaryWithData> = BTreeMap::new();
        for group in self.groups.iter() {
            for m in group.by_engine.values() {
                let e = map.entry(m.engine.clone()).or_insert_with(|| {
                    SummaryWithData {
                        name: m.engine.clone(),
                        version: m.engine_version.clone(),
                        ratios: vec![],
                    }
                });
                // OK because we know m.engine is in this group.
                let ratio = group.ratio(&m.engine, stat).unwrap();
                e.ratios.push(ratio);
            }
        }
        let mut summaries: Vec<EngineSummary> = map
            .into_iter()
            .map(|(_, summary)| {
                let mut geomean = 1.0;
                let count = summary.ratios.len();
                for &ratio in summary.ratios.iter() {
                    geomean *= ratio.powf(1.0 / count as f64);
                }

                EngineSummary {
                    name: summary.name,
                    version: summary.version,
                    geomean,
                    count,
                }
            })
            .collect();
        summaries.sort_by(|s1, s2| s1.geomean.total_cmp(&s2.geomean));
        Ok(summaries)
    }

    /// Returns a lexicographically sorted list of all regex engine names in
    /// this collection of measurements. The order is ascending.
    pub fn engine_names(&self) -> Vec<String> {
        let mut engine_names = BTreeSet::new();
        for group in self.groups.iter() {
            for m in group.by_engine.values() {
                engine_names.insert(m.engine.clone());
            }
        }
        engine_names.into_iter().collect()
    }
}

/// A single group of measurements, where every measurement has the same
/// benchmark name.
///
/// The type parameter refers to additional data attached to
/// this group. Usually it's either a `()` (for no data) or a
/// `crate::format::benchmarks::Definition`, where the definition corresponds
/// to the benchmark meta data for the group.
#[derive(Clone, Debug)]
pub struct ByBenchmarkNameGroup<T> {
    /// The name of the benchmark to which this group corresponds to. The
    /// invariant is that all of the measurements in `by_engine` have a
    /// benchmark name precisely equivalent to this.
    pub name: String,
    /// A map from engine name to the corresponding measurement, where every
    /// measurement in this group has the same benchmark name.
    pub by_engine: BTreeMap<String, Measurement>,
    /// Extra data attached to this group. Usually either `()` (for nothing),
    /// or the benchmark definition.
    pub data: T,
}

impl ByBenchmarkNameGroup<()> {
    /// Creates a new group of measurements.
    ///
    /// This panics if the group is empty or if there are other
    /// inconsistencies, such as if the benchmark name in any measurement
    /// doesn't match the name given.
    fn new(
        by_engine: BTreeMap<String, Measurement>,
    ) -> ByBenchmarkNameGroup<()> {
        let name = by_engine
            .values()
            .next()
            .expect("group must be non-empty")
            .name
            .clone();
        for m in by_engine.values() {
            assert_eq!(
                name, m.name,
                "expected all measurements in group to have benchmark \
                 name '{}', but found '{}' for engine '{}'",
                name, m.name, m.engine,
            );
        }
        ByBenchmarkNameGroup { name, by_engine, data: () }
    }
}

impl<T> ByBenchmarkNameGroup<T> {
    /// Return the ratio between the `this` engine and the best benchmark in
    /// the group. If `this` is the best, then the ratio returned is 1.0. Thus,
    /// the ratio is how many times slower this engine is from the best for
    /// this particular benchmark.
    ///
    /// This returns `None` if `this` does not correspond to an engine in this
    /// group.
    pub fn ratio(&self, this: &str, stat: Stat) -> Option<f64> {
        let this = self.by_engine.get(this)?.duration(stat).as_secs_f64();
        let best =
            self.by_engine[self.best(stat)].duration(stat).as_secs_f64();
        Some(this / best)
    }

    /// Returns true only when this group contains at least one aggregate
    /// measurement whose speedup ratio falls within the given range.
    ///
    /// The aggregate statistic used to test against the given range is
    /// specified by `stat`.
    pub fn is_within_range(&self, stat: Stat, range: ThresholdRange) -> bool {
        // We don't filter on the "best" engine below because its speedup ratio
        // is always 1. So if we have a group of size 1, then we don't filter
        // on spedup ratio at all and thus would return false below, which
        // doesn't seem right. So we detect that case and handle it specially
        // here.
        if self.by_engine.len() == 1 {
            return range.contains(1.0);
        }
        let best_engine = self.best(stat);
        let best = &self.by_engine[best_engine].duration(stat).as_secs_f64();
        for m in self.by_engine.values() {
            // The speedup ratio for the best engine is always 1.0, and so it
            // isn't useful to filter on it.
            if m.engine == best_engine {
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

    /// Return the engine name of the best measurement in this group. The name
    /// returned is guaranteed to exist in this group.
    pub fn best(&self, stat: Stat) -> &str {
        let mut it = self.by_engine.iter();
        // The unwrap is OK because our group is guaranteed to be non-empty.
        let mut best_engine = it.next().unwrap().0;
        for (engine, candidate) in self.by_engine.iter() {
            let best = &self.by_engine[best_engine];
            if candidate.duration(stat) < best.duration(stat) {
                best_engine = engine;
            }
        }
        best_engine
    }
}

/// A summary result for a single engine. Usually this only makes sense in the
/// context of summary results for other engines on the same measurement data.
#[derive(Clone, Debug)]
pub struct EngineSummary {
    /// The name of the regex engine, confirmed to be identical in all
    /// measurements that participated in this summary.
    pub name: String,
    /// The version of the regex engine, also confirmed to be identical in all
    /// measurements that participated in this summary.
    pub version: String,
    /// The geometric mean of the speedup ratios for this engine, relative
    /// to other engines, for every group of measurements for each unique
    /// benchmark name that this engine participated in.
    pub geomean: f64,
    /// The total number of unique benchmark names that contributed to the
    /// `geomean` result.
    pub count: usize,
}
