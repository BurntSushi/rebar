#![allow(warnings)]

use std::collections::{BTreeMap, BTreeSet};

use crate::{
    args::Stat,
    format::{
        benchmarks::{Benchmarks, Definition},
        measurement::Measurement,
    },
};

#[derive(Clone, Debug)]
pub struct Flattened {
    pub results: Vec<DefMeasurement>,
}

impl Flattened {
    pub fn new(
        benchmarks: Benchmarks,
        measurements: Vec<Measurement>,
    ) -> anyhow::Result<Flattened> {
        // Collect all of our definitions into a set keyed on (benchmark
        // name, engine name), and then ensure that every measurement has a
        // corresponding benchmark definition. This might not be true if the
        // measurements are stale and the definitions, e.g., got renamed or
        // removed.
        let mut set: BTreeSet<(String, String)> = BTreeSet::new();
        for def in benchmarks.defs.iter() {
            for engine in def.engines.iter() {
                set.insert((
                    def.name.as_str().to_string(),
                    engine.name.clone(),
                ));
            }
        }
        // While we check that every measurement has a corresponding
        // definition, we also group them by name and then by engine. This lets
        // us then associate each measurement with a definition.
        let mut map = BTreeMap::new();
        for m in measurements.iter() {
            if !set.contains(&(m.name.clone(), m.engine.clone())) {
                log::warn!(
                    "could not find '{}' and engine '{}' in set of benchmark \
                     definitions, so rebar will drop \
                     the measurement and continue",
                    m.name,
                    m.engine,
                );
                continue;
            }
            let result = map
                .entry(m.name.clone())
                .or_insert(BTreeMap::default())
                .insert(m.engine.clone(), m.clone());
            anyhow::ensure!(
                result.is_none(),
                "found measurement for benchmark '{}' with duplicative \
                 engine name '{}'",
                m.name,
                m.engine,
            );
        }
        // Finally associated each definition with a measurement to create
        // a sequence of flattened results.
        let mut flattened = Flattened { results: vec![] };
        for def in benchmarks.defs {
            // OK because we used a filter to select our benchmark definitions
            // that was derived from our measurements. So we shouldn't have
            // definitions that don't have a corresponding measurement.
            let measurements_by_engine = map.get(def.name.as_str()).unwrap();
            let mut defm =
                DefMeasurement { def, measurements: BTreeMap::new() };
            for engine in defm.def.engines.iter() {
                // It's possible for this to fail, because we might be missing
                // a measurement for a specific engine on one particular
                // benchmark, but not in others. We would have warned about
                // this previously.
                let m = match measurements_by_engine.get(&engine.name) {
                    Some(m) => m,
                    None => continue,
                };
                let result =
                    defm.measurements.insert(engine.name.clone(), m.clone());
                // This should never happen because the benchmark definition
                // format won't allow it and will return an error at load time.
                assert!(
                    result.is_none(),
                    "found benchmark '{}' with duplicate engine '{}'",
                    defm.def.name,
                    engine.name,
                );
            }
            flattened.results.push(defm);
        }
        Ok(flattened)
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
    pub fn engines(&self, stat: Stat) -> anyhow::Result<Vec<Engine>> {
        struct EngineDist {
            name: String,
            version: String,
            ratios_compile: Vec<f64>,
            ratios_search: Vec<f64>,
        }

        // Measurement data is just a flattened set of rows, so there is no
        // guarantee that the version remains the same for every regex engine.
        // So we explicitly check that invariant here.
        let mut map: BTreeMap<String, EngineDist> = BTreeMap::new();
        for defm in self.results.iter() {
            for m in defm.measurements.values() {
                let mut e = map.entry(m.engine.clone()).or_insert_with(|| {
                    EngineDist {
                        name: m.engine.clone(),
                        version: m.version.clone(),
                        ratios_compile: vec![],
                        ratios_search: vec![],
                    }
                });
                anyhow::ensure!(
                    e.version == m.version,
                    "found two different versions in measurements \
                         for engine '{}': '{}' and '{}'",
                    m.engine,
                    e.version,
                    m.version,
                );
                if m.model == "compile" {
                    e.ratios_compile.push(defm.ratio(&m.engine, stat));
                } else {
                    e.ratios_search.push(defm.ratio(&m.engine, stat));
                }
            }
        }
        let mut engines: Vec<Engine> = map
            .into_iter()
            .map(|(_, edist)| {
                let count_compile = edist.ratios_compile.len();
                let mut geomean_compile = 1.0;
                for &ratio in edist.ratios_compile.iter() {
                    geomean_compile *= ratio.powf(1.0 / count_compile as f64);
                }

                let mut geomean_search = 1.0;
                let count_search = edist.ratios_search.len();
                for &ratio in edist.ratios_search.iter() {
                    geomean_search *= ratio.powf(1.0 / count_search as f64);
                }

                Engine {
                    name: edist.name,
                    version: edist.version,
                    geomean_compile,
                    geomean_search,
                    count_compile,
                    count_search,
                }
            })
            .collect();
        // engines.sort_by(|e1, e2| e1.geomean.total_cmp(&e2.geomean));
        Ok(engines)
    }
}

/// An engine name and its version, along with some aggregate statistics
/// across all measurements, taken from the results and *not* the benchmark
/// definitions. That is, the versions represent the regex versions recorded at
/// the time of measurement and not the time of report generation.
#[derive(Clone, Debug)]
pub struct Engine {
    pub name: String,
    pub version: String,
    pub geomean_compile: f64,
    pub geomean_search: f64,
    pub count_compile: usize,
    pub count_search: usize,
}

#[derive(Clone, Debug)]
pub struct DefMeasurement {
    /// The definition of the benchmark that measurements were captured for.
    pub def: Definition,
    /// A map from engine name to the corresponding measurement for this
    /// benchmark.
    pub measurements: BTreeMap<String, Measurement>,
}

impl DefMeasurement {
    /// Return the ratio between the 'this' engine and the best benchmark in
    /// the group. The 'this' is the best, then the ratio returned is 1.0.
    /// Thus, the ratio is how many times slower this engine is from the best
    /// for this particular benchmark.
    pub fn ratio(&self, this: &str, stat: Stat) -> f64 {
        if self.measurements.len() < 2 {
            // I believe this is a redundant base case.
            return 1.0;
        }
        let this = self.measurements[this].duration(stat).as_secs_f64();
        let best =
            self.measurements[self.best(stat)].duration(stat).as_secs_f64();
        this / best
    }

    /// Return the engine name of the best measurement in this group. The name
    /// returned is guaranteed to exist in this group.
    pub fn best(&self, stat: Stat) -> &str {
        let mut it = self.measurements.iter();
        let mut best_engine = it.next().unwrap().0;
        for (engine, candidate) in self.measurements.iter() {
            let best = &self.measurements[best_engine];
            if candidate.duration(stat) < best.duration(stat) {
                best_engine = engine;
            }
        }
        best_engine
    }
}

/// A tree representation of results.
#[derive(Clone, Debug)]
pub enum Tree {
    Node { name: String, children: Vec<Tree> },
    Leaf(DefMeasurement),
}

impl Tree {
    /// Create a new tree of results from a flattened set of results.
    pub fn new(flattened: Flattened) -> Tree {
        let mut root = Tree::Node { name: String::new(), children: vec![] };
        for defm in flattened.results {
            root.add(defm);
        }
        root
    }

    /// Add the given definition measurement to this tree.
    fn add(&mut self, defm: DefMeasurement) {
        let mut node = self;
        for part in defm.def.name.group.split("/") {
            node = node.find_or_insert(part);
        }
        node.children().push(Tree::Leaf(defm));
    }

    /// Looks for a direct child node with the given name and returns it. If
    /// one could not be found, then one is inserted and that new node is
    /// returned.
    ///
    /// If this is a leaf node, then it panics.
    fn find_or_insert(&mut self, name: &str) -> &mut Tree {
        match *self {
            Tree::Leaf { .. } => unreachable!(),
            Tree::Node { ref mut children, .. } => {
                // This would be more naturally written as iterating over
                // 'children.iter_mut()' and just returning a child if one was
                // found, but I couldn't get the borrow checker to cooperate.
                let found = children.iter().position(|c| c.name() == name);
                let index = match found {
                    Some(index) => index,
                    None => {
                        let index = children.len();
                        children.push(Tree::Node {
                            name: name.to_string(),
                            children: vec![],
                        });
                        index
                    }
                };
                &mut children[index]
            }
        }
    }

    /// Returns the children of this internal tree node. If this is a leaf
    /// node, then it panics.
    fn children(&mut self) -> &mut Vec<Tree> {
        match *self {
            Tree::Leaf { .. } => unreachable!(),
            Tree::Node { ref mut children, .. } => children,
        }
    }

    /// Runs the given closure on every node in this tree in depth first order.
    /// This also skips any internal nodes that have no siblings. (In other
    /// words, any non-leafs that are singletons are flattened away because the
    /// presentation usually looks better without them.)
    pub fn flattened_depth_first(
        &self,
        mut f: impl FnMut(&Tree, usize) -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        fn imp(
            tree: &Tree,
            f: &mut impl FnMut(&Tree, usize) -> anyhow::Result<()>,
            siblings: usize,
            depth: usize,
        ) -> anyhow::Result<()> {
            match *tree {
                Tree::Leaf { .. } => f(tree, depth),
                Tree::Node { ref children, .. } => {
                    let depth = if siblings == 0
                        && !children.iter().all(Tree::is_leaf)
                    {
                        depth
                    } else {
                        f(tree, depth)?;
                        depth + 1
                    };
                    for c in children.iter() {
                        imp(c, f, children.len() - 1, depth)?;
                    }
                    Ok(())
                }
            }
        }
        imp(self, &mut f, 0, 0)
    }

    /// Returns the children of this node, but flattened. That is, if this
    /// node has only one non-leaf child, then it is skipped and the flattened
    /// children of that child are returned.
    ///
    /// This always returns an empty slice for leaf nodes.
    fn flattened_children(&self) -> &[Tree] {
        match *self {
            Tree::Leaf(_) => &[],
            Tree::Node { ref children, .. } => {
                if children.len() != 1 || children[0].is_leaf() {
                    return children;
                }
                children[0].flattened_children()
            }
        }
    }

    /// Returns true if and only if this is a leaf node.
    pub fn is_leaf(&self) -> bool {
        matches!(*self, Tree::Leaf { .. })
    }

    /// Returns true if and only if this is an internal node whose children
    /// are all leafs.
    fn is_parent_of_leaf(&self) -> bool {
        match *self {
            Tree::Leaf(_) => false,
            Tree::Node { ref children, .. } => {
                children.iter().all(Tree::is_leaf)
            }
        }
    }

    /// Returns the component name of this tree node.
    fn name(&self) -> &str {
        match *self {
            Tree::Node { ref name, .. } => name,
            Tree::Leaf(ref defm) => &defm.def.name.local,
        }
    }
}
