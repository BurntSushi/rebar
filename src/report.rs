#![allow(warnings)]

use std::{
    collections::{BTreeMap, BTreeSet},
    io::Write,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use {
    anyhow::Context,
    bstr::ByteSlice,
    lexopt::{Arg, ValueExt},
    once_cell::sync::Lazy,
    regex::Regex,
};

use crate::{
    args::{self, Filter, Stat, Units, Usage},
    format::{
        benchmarks::{Benchmarks, Definition, Engines, Filters},
        measurement::Measurement,
    },
    util::ShortHumanDuration,
};

const USAGES: &[Usage] = &[
    Usage::BENCH_DIR,
    Filter::USAGE_ENGINE,
    Filter::USAGE_BENCH,
    Filter::USAGE_MODEL,
    Usage::new(
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
    ),
    Usage::new(
        "--ratio",
        "Show ratios next to timings.",
        r#"
Show ratios next to timings in result tables.

This is useful for giving a quick relative comparison between results, but
it also tends to bloat the size of the table and overall makes it a bit more
noisy.
"#,
    ),
    Stat::USAGE,
    Usage::new(
        "--summary-exclude",
        "A pattern for excluding engines from the summary table.",
        r#"
A pattern for excluding engines from the summary table.

This is useful in cases where a regex engine participates in one or two
benchmarks, but generally should not be included in the overall ranking of
regex engines. In particular, being in so few benchmarks can ultimately skew
the overall ranking in a way that makes it very confusing to interpret.

Note that this doesn't impact the geometric means computed for other regex
engines. For example, if an excluded regex engine did the best in a benchmark,
then other engines in that benchmark will have a speed ratio above 1.
"#,
    ),
    Units::USAGE,
];

const SPLICE_BEGIN: &str = "<!-- BEGIN: report -->";
const SPLICE_END: &str = "<!-- END: report -->";

fn usage() -> String {
    format!(
        "\
Print a Markdown formatted report of results for a group of benchmarks.

The primary input for this command is one or more CSV files that were generated
by the 'rebar measure' command. There must not be any duplicate benchmarks
among the files, or else this command will report an error.

The --splice flag can be used to print the report into an existing Markdown
file. Splicing works by finding removing all lines between

    <!-- BEGIN: report -->

and

    <!-- END: report -->

and then replacing them with the lines making up the report.

By default, this command will generate information about every benchmark
represented in the results given. Filters can be used to select only a subset
of benchmarks to include in the report.

For example, these are the commands used to generate the report in rebar's
README. First, we run the benchmarks:

    rebar measure -f '^curated/' > path/to/results.csv

and then we generate the report from the results:

    rebar report --splice README.md path/to/results.csv

USAGE:
    rebar report [options] <csv-path> ...

OPTIONS:
{options}
",
        options = Usage::short(USAGES),
    )
    .trim()
    .to_string()
}

pub fn run(p: &mut lexopt::Parser) -> anyhow::Result<()> {
    let config = Config::parse(p)?;
    let measurements = config.read_measurements()?;
    let benchmarks = config.read_benchmarks(&measurements)?;
    let engines_defined = benchmarks.engines.clone();
    let analysis = benchmarks.analysis.clone();
    let flattened = Flattened::new(benchmarks, measurements)?;
    let engines_measured = flattened.engines(&config)?;
    let tree = Tree::new(flattened);
    let mut out = vec![];
    markdown(
        &config,
        &engines_defined,
        &engines_measured,
        &analysis,
        &tree,
        &mut out,
    )?;
    if let Some(ref path) = config.splice {
        splice(path, &out)?;
    } else {
        std::io::stdout().write_all(&out)?;
    }
    Ok(())
}

/// The arguments for this 'report' command parsed from CLI args.
#[derive(Debug, Default)]
struct Config {
    /// File paths to CSV files.
    csv_paths: Vec<PathBuf>,
    /// The directory to find benchmark definitions and haystacks.
    dir: PathBuf,
    /// A Markdown file to splice the report into.
    splice: Option<PathBuf>,
    /// A filter to be applied to benchmark "full names."
    bench_filter: Filter,
    /// A filter to be applied to regex engine names.
    engine_filter: Filter,
    /// A filter to be applied to benchmark model name.
    model_filter: Filter,
    /// Whether to only consider benchmarks containing all regex engines.
    intersection: bool,
    /// The statistic we want to compare.
    stat: Stat,
    /// A pattern for excluding regex engines from the summary table.
    summary_exclude: Option<Regex>,
    /// The statistical units we want to use in our comparisons.
    units: Units,
    /// Whether to show ratios with timings.
    ratio: bool,
}

impl Config {
    /// Parse 'cmp' args from the given CLI parser.
    fn parse(p: &mut lexopt::Parser) -> anyhow::Result<Config> {
        use lexopt::Arg;

        let mut c = Config::default();
        c.dir = PathBuf::from("benchmarks");
        while let Some(arg) = p.next()? {
            match arg {
                Arg::Value(v) => c.csv_paths.push(PathBuf::from(v)),
                Arg::Short('h') => anyhow::bail!("{}", usage()),
                Arg::Long("help") => anyhow::bail!("{}", usage()),
                Arg::Short('d') | Arg::Long("dir") => {
                    c.dir = PathBuf::from(p.value().context("-d/--dir")?);
                }
                Arg::Short('e') | Arg::Long("engine") => {
                    c.engine_filter.add(args::parse(p, "-e/--engine")?);
                }
                Arg::Short('f') | Arg::Long("filter") => {
                    c.bench_filter.add(args::parse(p, "-f/--filter")?);
                }
                Arg::Short('m') | Arg::Long("model") => {
                    c.model_filter.add(args::parse(p, "-m/--model")?);
                }
                Arg::Long("intersection") => {
                    c.intersection = true;
                }
                Arg::Long("ratio") => {
                    c.ratio = true;
                }
                Arg::Long("splice") => {
                    c.splice =
                        Some(PathBuf::from(p.value().context("--splice")?));
                }
                Arg::Short('s') | Arg::Long("statistic") => {
                    c.stat = args::parse(p, "-s/--statistic")?;
                }
                Arg::Long("summary-exclude") => {
                    let value = p.value().context("--summary-exclude")?;
                    let pat = value.string().context("--summary-exclude")?;
                    let re = Regex::new(&pat).context("--summary-exclude")?;
                    c.summary_exclude = Some(re);
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

    /// Read and parse benchmark definitions from TOML files in the --dir
    /// directory.
    ///
    /// This uses the given measurements to setup a filter that only reads
    /// benchmark definitions (and engines) for the measurements given.
    fn read_benchmarks(
        &self,
        measurements: &[Measurement],
    ) -> anyhow::Result<Benchmarks> {
        let mut engine_names: Vec<String> = measurements
            .iter()
            .map(|m| regex_syntax::escape(&m.engine))
            .collect();
        engine_names.sort();
        engine_names.dedup();
        let pat = format!("^(?:{})$", engine_names.join("|"));
        let mut engine_filter = Filter::from_pattern(&pat)
            .context("failed to build filter for engine names")?;

        let mut bench_names: Vec<String> = measurements
            .iter()
            .map(|m| regex_syntax::escape(&m.name))
            .collect();
        let pat = format!("^(?:{})$", bench_names.join("|"));
        let mut bench_filter = Filter::from_pattern(&pat)
            .context("failed to build filter for benchmark names")?;

        let mut filters = Filters::new();
        filters
            .name(bench_filter)
            .engine(engine_filter)
            .ignore_missing_engines(true);
        let mut benchmarks = Benchmarks::from_dir(&self.dir, &filters)?;
        // Sort benchmarks by their group name so that they appear in a
        // consistent order. We retain the order of benchmarks within a
        // group, since that order always corresponds to the order they were
        // originally defined in a TOML file. (And that's why it's important to
        // do a stable sort here.)
        benchmarks.defs.sort_by(|d1, d2| d1.name.group.cmp(&d2.name.group));
        Ok(benchmarks)
    }

    /// Reads all aggregate benchmark measurements from all CSV file paths
    /// given, and returns them as one flattened vector. The filters provided
    /// to the CLI are applied. If any duplicates are seen (for a given
    /// benchmark name and regex engine pair), then an error is returned.
    fn read_measurements(&self) -> anyhow::Result<Vec<Measurement>> {
        let mut measurements = vec![];
        // A set of (benchmark full name, regex engine name) pairs.
        // let mut seen: BTreeSet<(String, String)> = BTreeSet::new();
        // A map from benchmark full name to the set of regex engines
        // for which we have measurements.
        let mut name_to_engines: BTreeMap<String, BTreeSet<String>> =
            BTreeMap::new();
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
                if !name_to_engines.contains_key(&m.name) {
                    name_to_engines.insert(m.name.clone(), BTreeSet::new());
                }
                let is_new = name_to_engines
                    .entry(m.name.clone())
                    .or_insert_with(|| BTreeSet::new())
                    .insert(m.engine.clone());
                anyhow::ensure!(
                    is_new,
                    "duplicate measurement with name {} and regex engine {}",
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

#[derive(Clone, Debug)]
struct Flattened {
    results: Vec<DefMeasurement>,
}

impl Flattened {
    fn new(
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
    fn engines(&self, c: &Config) -> anyhow::Result<Vec<Engine>> {
        #[derive(Debug)]
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
                    e.ratios_compile.push(defm.ratio(&m.engine, c.stat));
                } else {
                    e.ratios_search.push(defm.ratio(&m.engine, c.stat));
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
        Ok(engines)
    }
}

/// An engine name and its version, along with some aggregate statistics
/// across all measurements, taken from the results and *not* the benchmark
/// definitions. That is, the versions represent the regex versions recorded at
/// the time of measurement and not the time of report generation.
#[derive(Clone, Debug)]
struct Engine {
    name: String,
    version: String,
    geomean_compile: f64,
    geomean_search: f64,
    count_compile: usize,
    count_search: usize,
}

#[derive(Clone, Debug)]
struct DefMeasurement {
    /// The definition of the benchmark that measurements were captured for.
    def: Definition,
    /// A map from engine name to the corresponding measurement for this
    /// benchmark.
    measurements: BTreeMap<String, Measurement>,
}

impl DefMeasurement {
    /// Return the ratio between the 'this' engine and the best benchmark in
    /// the group. The 'this' is the best, then the ratio returned is 1.0.
    /// Thus, the ratio is how many times slower this engine is from the best
    /// for this particular benchmark.
    fn ratio(&self, this: &str, stat: Stat) -> f64 {
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
    fn best(&self, stat: Stat) -> &str {
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
enum Tree {
    Node { name: String, children: Vec<Tree> },
    Leaf(DefMeasurement),
}

impl Tree {
    /// Create a new tree of results from a flattened set of results.
    fn new(flattened: Flattened) -> Tree {
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
    fn flattened_depth_first(
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
    fn is_leaf(&self) -> bool {
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

fn markdown<W: Write>(
    config: &Config,
    engines_defined: &Engines,
    engines_measured: &[Engine],
    analysis: &BTreeMap<String, String>,
    tree: &Tree,
    mut wtr: W,
) -> anyhow::Result<()> {
    writeln!(wtr, "<!-- Auto-generated by rebar, do not edit manually! -->")?;
    writeln!(wtr, "<!-- Generated with command: -->")?;
    write!(wtr, "<!--")?;
    for arg in std::env::args() {
        write!(wtr, " {}", arg)?;
    }
    writeln!(wtr, " -->")?;

    markdown_summary(config, engines_defined, engines_measured, &mut wtr)?;
    markdown_bench_list(tree, &mut wtr)?;
    markdown_results(config, analysis, tree, &mut wtr)?;
    Ok(())
}

fn markdown_bench_list<W: Write>(
    tree: &Tree,
    mut wtr: W,
) -> anyhow::Result<()> {
    let explanation = format!(
        r#"
Below is a list of links to each benchmark group in this particular barometer.
Each benchmark group contains 1 or more related benchmarks. The idea of each
group is to tell some kind of story about related workloads, and to give
a sense of how performance changes based on the variations between each
benchmark.
"#,
    );
    writeln!(wtr, "### Benchmark Groups")?;
    writeln!(wtr, "")?;
    writeln!(wtr, "{}", explanation.trim())?;
    writeln!(wtr, "")?;
    tree.flattened_depth_first(|tree, depth| {
        let indent = "  ".repeat(depth);
        match *tree {
            Tree::Leaf(ref defm) => {}
            Tree::Node { ref name, ref children } => {
                let nice_name = nice_name(name);
                writeln!(wtr, "{}* [{}](#{})", indent, nice_name, nice_name)?;
            }
        }
        Ok(())
    })?;
    writeln!(wtr, "")?;
    Ok(())
}

fn markdown_summary<W: Write>(
    config: &Config,
    engines_defined: &Engines,
    engines_measured: &[Engine],
    mut wtr: W,
) -> anyhow::Result<()> {
    let explanation = format!(
        r#"
Below are two tables summarizing the results of regex engines benchmarked.
Each regex engine includes its version at the time measurements were captured,
a summary score that ranks it relative to other regex engines across all
benchmarks and the total number of measurements collected.

The first table ranks regex engines based on search time. The second table
ranks regex engines based on compile time.

The summary statistic used is the [geometric mean] of the speed ratios for
each regex engine across all benchmarks that include it. The ratios within
each benchmark are computed from the {stat} of all timing samples taken, and
dividing it by the best {stat} of the regex engines that participated in the
benchmark. For example, given two regex engines `A` and `B` with results `35
ns` and `25 ns` on a single benchmark, `A` has a speed ratio of `1.4` and
`B` has a speed ratio of `1.0`. The geometric mean reported here is then the
"average" speed ratio for that regex engine across all benchmarks.

Each regex engine is linked to the directory containing the runner program
responsible for compiling a regex, using it in a search and reporting timing
results. Each directory contains a `README` file briefly describing any engine
specific details for the runner program.

Each regex engine is also defined in
[benchmarks/engines.toml](benchmarks/engines.toml), using the same name listed
in the table below. Each definition includes instructions for how to run,
build, clean and obtain the version of each regex engine.

**Caution**: Using a single number to describe the overall performance of a
regex engine is a fraught endeavor, and it is debatable whether it should be
included here at all. It is included primarily because the number of benchmarks
is quite large and overwhelming. It can be quite difficult to get a general
sense of things without a summary statistic. In particular, a summary statistic
is also useful to observe how the _overall picture_ itself changes as changes
are made to the barometer. (Whether it be by adding new regex engines or
adding/removing/changing existing benchmarks.) One particular word of caution
is that while geometric mean is more robust with respect to outliers than
arithmetic mean, it is not unaffected by them. Therefore, it is still critical
to examine individual benchmarks if one wants to better understanding the
performance profile of any specific regex engine or workload.

[geometric mean]: https://dl.acm.org/doi/pdf/10.1145/5666.5673
"#,
        stat = config.stat,
    );

    writeln!(wtr, "### Summary")?;
    writeln!(wtr, "")?;
    writeln!(wtr, "{}", explanation.trim())?;
    writeln!(wtr, "")?;

    writeln!(wtr, "#### Summary of search-time benchmarks")?;
    writeln!(wtr, "")?;
    writeln!(wtr, "| Engine | Version | Geometric mean of speed ratios | Benchmark count |")?;
    writeln!(wtr, "| ------ | ------- | ------------------------------ | --------------- |")?;
    let mut measured = engines_measured.to_vec();
    measured.sort_by(|e1, e2| e1.geomean_search.total_cmp(&e2.geomean_search));
    for emeasured in measured.iter() {
        if let Some(ref re) = config.summary_exclude {
            if re.is_match(&emeasured.name) {
                continue;
            }
        }
        if emeasured.count_search == 0 {
            continue;
        }
        write!(wtr, "| ")?;
        // We want to link to the directory containing the runner program
        // for each engine, but this relies on 'cwd' being set in the engine
        // definition. It might not be. It's not required. But in practice, all
        // do it.
        let linkdir = engines_defined
            .by_name
            .get(&emeasured.name)
            .and_then(|e| e.run.cwd.as_ref());
        match linkdir {
            None => write!(wtr, "{}", emeasured.name)?,
            Some(dir) => write!(wtr, "[{}]({})", emeasured.name, dir)?,
        }
        writeln!(
            wtr,
            " | {} | {:.2} | {} |",
            emeasured.version,
            emeasured.geomean_search,
            emeasured.count_search,
        )?;
    }
    writeln!(wtr, "")?;

    writeln!(wtr, "#### Summary of compile-time benchmarks")?;
    writeln!(wtr, "")?;
    writeln!(wtr, "| Engine | Version | Geometric mean of speed ratios | Benchmark count |")?;
    writeln!(wtr, "| ------ | ------- | ------------------------------ | --------------- |")?;
    let mut measured = engines_measured.to_vec();
    measured
        .sort_by(|e1, e2| e1.geomean_compile.total_cmp(&e2.geomean_compile));
    for emeasured in measured.iter() {
        if let Some(ref re) = config.summary_exclude {
            if re.is_match(&emeasured.name) {
                continue;
            }
        }
        if emeasured.count_compile == 0 {
            continue;
        }
        write!(wtr, "| ")?;
        // We want to link to the directory containing the runner program
        // for each engine, but this relies on 'cwd' being set in the engine
        // definition. It might not be. It's not required. But in practice, all
        // do it.
        let linkdir = engines_defined
            .by_name
            .get(&emeasured.name)
            .and_then(|e| e.run.cwd.as_ref());
        match linkdir {
            None => write!(wtr, "{}", emeasured.name)?,
            Some(dir) => write!(wtr, "[{}]({})", emeasured.name, dir)?,
        }
        writeln!(
            wtr,
            " | {} | {:.2} | {} |",
            emeasured.version,
            emeasured.geomean_compile,
            emeasured.count_compile,
        )?;
    }
    writeln!(wtr, "")?;

    Ok(())
}

fn markdown_results<W: Write>(
    config: &Config,
    analysis: &BTreeMap<String, String>,
    tree: &Tree,
    mut wtr: W,
) -> anyhow::Result<()> {
    tree.flattened_depth_first(|tree, depth| {
        match *tree {
            Tree::Leaf { .. } => {}
            Tree::Node { ref name, ref children } => {
                let header = "#".repeat(depth + 3);
                let nice_name = nice_name(name);
                writeln!(wtr, "{} {}", header, nice_name)?;
                writeln!(wtr, "")?;
                if children.iter().all(Tree::is_leaf) {
                    let mut defms = vec![];
                    for c in children.iter() {
                        let defm = match *c {
                            Tree::Leaf(ref defm) => defm,
                            Tree::Node { .. } => unreachable!(),
                        };
                        defms.push(defm);
                    }
                    markdown_result_group(config, analysis, &defms, &mut wtr)?
                }
            }
        }
        Ok(())
    })
}

fn markdown_result_group<W: Write>(
    config: &Config,
    analysis: &BTreeMap<String, String>,
    defms: &[&DefMeasurement],
    wtr: &mut W,
) -> anyhow::Result<()> {
    if defms.is_empty() {
        writeln!(wtr, "NO MEASUREMENTS TO REPORT")?;
        return Ok(());
    }
    if let Some(ref analysis) = analysis.get(&defms[0].def.name.group) {
        writeln!(wtr, "{}", analysis.trim());
        writeln!(wtr, "")?;
    }

    write!(wtr, "| Engine |")?;
    for defm in defms.iter() {
        write!(wtr, " {} |", defm.def.name.local)?;
    }
    writeln!(wtr, "")?;
    write!(wtr, "| - |")?;
    for defm in defms.iter() {
        write!(wtr, " - |")?;
    }
    writeln!(wtr, "")?;

    let mut engines = BTreeSet::new();
    for defm in defms.iter() {
        for e in defm.measurements.keys() {
            engines.insert(e.clone());
        }
    }
    for e in engines.iter() {
        write!(wtr, "| {} |", e)?;
        for defm in defms.iter() {
            let m = match defm.measurements.get(e) {
                None => {
                    write!(wtr, " - |")?;
                    continue;
                }
                Some(m) => m,
            };
            write!(wtr, " ")?;
            let ratio = defm.ratio(e, config.stat);
            let is_best = e == defm.best(config.stat);
            if is_best {
                write!(wtr, "**")?;
            }
            match config.units {
                Units::Throughput if m.aggregate.tputs.is_some() => {
                    let tput = m.throughput(config.stat).unwrap();
                    write!(wtr, "{}", tput)?;
                }
                _ => {
                    let d = m.duration(config.stat);
                    let humand = ShortHumanDuration::from(d);
                    write!(wtr, "{}", humand)?;
                }
            }
            if config.ratio {
                write!(wtr, " ({:.2}x)", ratio)?;
            }
            if is_best {
                write!(wtr, "**")?;
            }
            write!(wtr, " |")?;
        }
        writeln!(wtr, "")?;
    }
    writeln!(wtr, "")?;

    writeln!(wtr, "<details>")?;
    writeln!(wtr, "<summary>Show individual benchmark parameters.</summary>")?;
    writeln!(wtr, "")?;
    for defm in defms.iter() {
        writeln!(wtr, "**{}**", defm.def.name.local)?;
        writeln!(wtr, "")?;

        writeln!(wtr, "| Parameter | Value |")?;
        writeln!(wtr, "| --------- | ----- |")?;
        writeln!(wtr, "| full name | `{}` |", defm.def.name)?;
        writeln!(
            wtr,
            "| model | [`{model}`](MODELS.md#{model}) |",
            model = defm.def.model.as_str()
        )?;
        if let Some(ref path) = defm.def.regex_path {
            writeln!(
                wtr,
                "| regex-path | [`{path}`](benchmarks/regexes/{path}) |",
                path = path,
            )?;
        } else if defm.def.regexes.is_empty() {
            writeln!(wtr, "| regex | NONE |")?;
        } else if defm.def.regexes.len() == 1 {
            writeln!(
                wtr,
                "| regex | `````{}````` |",
                markdown_table_escape(&defm.def.regexes[0])
            )?;
        } else {
            for (i, re) in defm.def.regexes.iter().enumerate() {
                writeln!(
                    wtr,
                    "| regex({}) | `````{}````` |",
                    i,
                    markdown_table_escape(re)
                )?;
            }
        }
        writeln!(
            wtr,
            "| case-insensitive | `{}` |",
            defm.def.options.case_insensitive
        )?;
        writeln!(wtr, "| unicode | `{}` |", defm.def.options.unicode)?;
        if let Some(ref path) = defm.def.haystack_path {
            writeln!(
                wtr,
                "| haystack-path | [`{path}`](benchmarks/haystacks/{path}) |",
                path = path
            );
        } else {
            const LIMIT: usize = 60;
            write!(wtr, "| haystack | ")?;
            let haystack = &defm.def.haystack;
            if haystack.len() > LIMIT {
                write!(wtr, "`{} [.. snip ..]`", haystack[..LIMIT].as_bstr())?;
            } else {
                write!(wtr, "`{}`", haystack.as_bstr());
            }
            writeln!(wtr, " |")?;
        }
        for ec in defm.def.count.iter() {
            writeln!(
                wtr,
                "| count(`{}`) | {} |",
                ec.engine,
                // engine.replace("*", r"\*"),
                ec.count,
            )?;
        }

        writeln!(wtr, "")?;
        if let Some(ref analysis) = defm.def.analysis {
            writeln!(wtr, "{}", analysis.trim())?;
        }
        writeln!(wtr, "")?;
    }
    writeln!(wtr, "</details>")?;
    writeln!(wtr, "")?;
    Ok(())
}

fn markdown_table_escape(v: &str) -> String {
    v.replace("|", r"\|")
}

/// Splices the given report into the given file path. This returns an error
/// if reading or writing the file fails, or if appropriate begin and end
/// markers for the report could not be found.
fn splice(path: &Path, report: &[u8]) -> anyhow::Result<()> {
    static RE: Lazy<regex::bytes::Regex> = Lazy::new(|| {
        regex::bytes::Regex::new(
            r"\n<!-- BEGIN: report -->\n((?s:.*?))<!-- END: report -->\n",
        )
        .unwrap()
    });
    let src =
        std::fs::read(path).with_context(|| path.display().to_string())?;
    let remove = match RE.captures(&src) {
        None => anyhow::bail!("could not find report markers in splice file"),
        Some(caps) => caps.get(1).unwrap(),
    };
    let mut out = vec![];
    out.extend_from_slice(&src[..remove.start()]);
    out.extend_from_slice(report);
    out.extend_from_slice(&src[remove.end()..]);
    std::fs::write(path, &out).with_context(|| path.display().to_string())?;
    Ok(())
}

/// Formats the name of something by applying various conventions used in
/// benchmark definitions.
fn nice_name(name: &str) -> String {
    static RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^([0-9]+-)").unwrap());
    RE.replace(name, "").into_owned()
}
