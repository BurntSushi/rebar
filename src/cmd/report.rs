use std::{
    collections::{BTreeMap, BTreeSet},
    io::Write,
    path::{Path, PathBuf},
};

use {anyhow::Context, bstr::ByteSlice, lexopt::ValueExt, regex_lite::Regex};

use crate::{
    args::{self, Filter, Filters, Stat, Units, Usage},
    format::{
        benchmarks::{Benchmarks, Definition, Engines},
        measurement::{Measurement, MeasurementReader},
    },
    grouped::{ByBenchmarkName, ByBenchmarkNameGroup, EngineSummary},
    util::{self, ShortHumanDuration},
};

const USAGES: &[Usage] = &[
    Usage::BENCH_DIR,
    Filter::USAGE_ENGINE,
    Filter::USAGE_ENGINE_NOT,
    Filter::USAGE_BENCH,
    Filter::USAGE_BENCH_NOT,
    MeasurementReader::USAGE_INTERSECTION,
    Filter::USAGE_MODEL,
    Filter::USAGE_MODEL_NOT,
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
    Usage::new(
        "--relative-path-to-repo-root <path>",
        "Set the relative path to the repo root, used for hyperlinks.",
        r#"
This flag sets the relative path to the repo root, where the path is relative
to where the report lives. By default, the path is empty, which implies that
the report lives in the repo root.

A non-empty path should end with a `/`.

For a report in, for example, record/all/2023-04-11/README.md, the relative
path to the repo root should be `../../../`.
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

fn usage_short() -> String {
    format!(
        "\
Print a Markdown formatted report of results for a group of benchmarks.

USAGE:
    rebar report [options] <csv-path> ...

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
    let benchmarks = config.read_benchmarks(&measurements)?;
    let engines = benchmarks.engines.clone();
    let analysis = benchmarks.analysis.clone();
    let grouped =
        ByBenchmarkName::new(&measurements)?.associate(benchmarks.defs)?;
    let tree = Tree::new(grouped.clone());
    let mut out = vec![];
    markdown(&config, &engines, grouped, &analysis, &tree, &mut out)?;
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
    /// The benchmark name, model and regex engine filters.
    filters: Filters,
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
    /// Relative path to the repository root.
    relative_path_root: String,
}

impl Config {
    /// Parse 'cmp' args from the given CLI parser.
    fn parse(p: &mut lexopt::Parser) -> anyhow::Result<Config> {
        use lexopt::Arg;

        let mut c = Config::default();
        c.dir = PathBuf::from("benchmarks");
        c.filters.ignore_missing_engines = true;
        while let Some(arg) = p.next()? {
            match arg {
                Arg::Value(v) => c.csv_paths.push(PathBuf::from(v)),
                Arg::Short('h') => anyhow::bail!("{}", usage_short()),
                Arg::Long("help") => anyhow::bail!("{}", usage_long()),
                Arg::Short('d') | Arg::Long("dir") => {
                    c.dir = PathBuf::from(p.value().context("-d/--dir")?);
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
                Arg::Long("ratio") => {
                    c.ratio = true;
                }
                Arg::Long("relative-path-to-repo-root") => {
                    let value =
                        p.value().context("--relative-path-to-repo-root")?;
                    c.relative_path_root = value
                        .string()
                        .context("--relative-path-to-repo-root")?;
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
            .map(|m| regex_lite::escape(&m.engine))
            .collect();
        engine_names.sort();
        engine_names.dedup();
        let pat = format!("^(?:{})$", engine_names.join("|"));
        let engine_filter = Filter::from_pattern(&pat)
            .context("failed to build filter for engine names")?;

        let bench_names: Vec<String> =
            measurements.iter().map(|m| regex_lite::escape(&m.name)).collect();
        let pat = format!("^(?:{})$", bench_names.join("|"));
        let bench_filter = Filter::from_pattern(&pat)
            .context("failed to build filter for benchmark names")?;

        let mut benchmarks = Benchmarks::from_dir(
            &self.dir,
            &Filters {
                name: bench_filter,
                engine: engine_filter,
                model: Filter::default(),
                ignore_missing_engines: true,
            },
        )?;
        // Sort benchmarks by their group name so that they appear in a
        // consistent order. We retain the order of benchmarks within a
        // group, since that order always corresponds to the order they were
        // originally defined in a TOML file. (And that's why it's important to
        // do a stable sort here.)
        benchmarks.defs.sort_by(|d1, d2| d1.name.group.cmp(&d2.name.group));
        Ok(benchmarks)
    }

    /// Returns a Markdown link to another document within this repository
    /// with the given display text and URL.
    ///
    /// The URL is automatically prepended with the relative path to the root.
    /// That means the URL given here should be written as-if it were from the
    /// root.
    fn url(&self, display: &str, path: &str) -> String {
        let path = format!("{}{}", self.relative_path_root, path);
        format!("[{}]({})", display, path)
    }
}

/// A tree representation of results.
#[derive(Clone, Debug)]
enum Tree {
    Node { name: String, children: Vec<Tree> },
    Leaf(ByBenchmarkNameGroup<Definition>),
}

impl Tree {
    /// Create a new tree of results from a flattened set of results.
    fn new(by_name: ByBenchmarkName<Definition>) -> Tree {
        let mut root = Tree::Node { name: String::new(), children: vec![] };
        for group in by_name.groups {
            root.add(group);
        }
        root
    }

    /// Add the given definition measurement to this tree.
    fn add(&mut self, group: ByBenchmarkNameGroup<Definition>) {
        let mut node = self;
        for part in group.data.name.group.split("/") {
            node = node.find_or_insert(part);
        }
        node.children().push(Tree::Leaf(group));
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

    /// Returns true if and only if this is a leaf node.
    fn is_leaf(&self) -> bool {
        matches!(*self, Tree::Leaf { .. })
    }

    /// Returns the component name of this tree node.
    fn name(&self) -> &str {
        match *self {
            Tree::Node { ref name, .. } => name,
            Tree::Leaf(ref group) => &group.data.name.local,
        }
    }
}

fn markdown<W: Write>(
    config: &Config,
    engines: &Engines,
    grouped: ByBenchmarkName<Definition>,
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

    markdown_summary(config, engines, grouped, &mut wtr)?;
    markdown_bench_list(tree, &mut wtr)?;
    markdown_results(config, analysis, tree, &mut wtr)?;
    Ok(())
}

fn markdown_bench_list<W: Write>(
    tree: &Tree,
    mut wtr: W,
) -> anyhow::Result<()> {
    let revision = match util::REBAR_REVISION {
        None => "".to_string(),
        Some(rev) => format!(" (rev {})", rev),
    };
    let explanation = format!(
        r#"
Below is a list of links to each benchmark group in this particular barometer.
Each benchmark group contains 1 or more related benchmarks. The idea of each
group is to tell some kind of story about related workloads, and to give
a sense of how performance changes based on the variations between each
benchmark.

This report was generated by `rebar {version}{revision}`.
"#,
        version = util::REBAR_VERSION,
        revision = revision,
    );
    writeln!(wtr, "### Benchmark Groups")?;
    writeln!(wtr, "")?;
    writeln!(wtr, "{}", explanation.trim())?;
    writeln!(wtr, "")?;
    tree.flattened_depth_first(|tree, depth| {
        let indent = "  ".repeat(depth);
        match *tree {
            Tree::Leaf(_) => {}
            Tree::Node { ref name, .. } => {
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
    engines: &Engines,
    grouped: ByBenchmarkName<Definition>,
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

If you're looking to compare two regex engines specifically, then it is better
to do so based only on the benchmarks that they both participate in. For
example, to compared based on the results recorded on 2023-05-04, one can do:

```
$ rebar rank record/all/2023-05-04/*.csv -f '^curated/' -e '^(rust/regex|hyperscan)$' --intersection -M compile
Engine      Version           Geometric mean of speed ratios  Benchmark count
------      -------           ------------------------------  ---------------
hyperscan   5.4.1 2023-02-22  2.03                            25
rust/regex  1.8.1             2.13                            25
```

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

    let (grouped_compile, grouped_search) =
        grouped.partition(|g| g.data.model == "compile");
    let ranked_compile: Vec<EngineSummary> = grouped_compile
        .ranking(config.stat)?
        .into_iter()
        .filter(|s| s.count > 0)
        .filter(|s| {
            config
                .summary_exclude
                .as_ref()
                .map_or(true, |re| !re.is_match(&s.name))
        })
        .collect();
    let ranked_search: Vec<EngineSummary> = grouped_search
        .ranking(config.stat)?
        .into_iter()
        .filter(|s| s.count > 0)
        .filter(|s| {
            config
                .summary_exclude
                .as_ref()
                .map_or(true, |re| !re.is_match(&s.name))
        })
        .collect();

    if !ranked_compile.is_empty() || !ranked_search.is_empty() {
        writeln!(wtr, "### Summary")?;
        writeln!(wtr, "")?;
        writeln!(wtr, "{}", explanation.trim())?;
        writeln!(wtr, "")?;

        if !ranked_search.is_empty() {
            writeln!(wtr, "#### Summary of search-time benchmarks")?;
            writeln!(wtr, "")?;
            markdown_summary_table(config, engines, &ranked_search, &mut wtr)?;
        }
        if !ranked_compile.is_empty() {
            writeln!(wtr, "#### Summary of compile-time benchmarks")?;
            writeln!(wtr, "")?;
            markdown_summary_table(
                config,
                engines,
                &ranked_compile,
                &mut wtr,
            )?;
        }
    }

    Ok(())
}

fn markdown_summary_table<W: Write>(
    config: &Config,
    engines: &Engines,
    summaries: &[EngineSummary],
    mut wtr: W,
) -> anyhow::Result<()> {
    writeln!(wtr, "| Engine | Version | Geometric mean of speed ratios | Benchmark count |")?;
    writeln!(wtr, "| ------ | ------- | ------------------------------ | --------------- |")?;
    for summary in summaries.iter() {
        if summary.count == 0 {
            continue;
        }
        write!(wtr, "| ")?;
        // We want to link to the directory containing the runner program
        // for each engine, but this relies on 'cwd' being set in the engine
        // definition. It might not be. It's not required. But in practice, all
        // do it.
        let linkdir = engines
            .by_name
            .get(&summary.name)
            .and_then(|e| e.run.cwd.as_ref());
        match linkdir {
            None => write!(wtr, "{}", summary.name)?,
            Some(dir) => write!(wtr, "{}", config.url(&summary.name, dir))?,
        }
        writeln!(
            wtr,
            " | {} | {:.2} | {} |",
            summary.version, summary.geomean, summary.count,
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
    groups: &[&ByBenchmarkNameGroup<Definition>],
    wtr: &mut W,
) -> anyhow::Result<()> {
    if groups.is_empty() {
        writeln!(wtr, "NO MEASUREMENTS TO REPORT")?;
        return Ok(());
    }
    if let Some(ref analysis) = analysis.get(&groups[0].data.name.group) {
        writeln!(wtr, "{}", analysis.trim())?;
        writeln!(wtr, "")?;
    }

    write!(wtr, "| Engine |")?;
    for group in groups.iter() {
        write!(wtr, " {} |", group.data.name.local)?;
    }
    writeln!(wtr, "")?;
    write!(wtr, "| - |")?;
    for _ in groups.iter() {
        write!(wtr, " - |")?;
    }
    writeln!(wtr, "")?;

    let mut engines = BTreeSet::new();
    for group in groups.iter() {
        for e in group.by_engine.keys() {
            engines.insert(e.clone());
        }
    }
    for e in engines.iter() {
        write!(wtr, "| {} |", e)?;
        for group in groups.iter() {
            let m = match group.by_engine.get(e) {
                None => {
                    write!(wtr, " - |")?;
                    continue;
                }
                Some(m) => m,
            };
            write!(wtr, " ")?;
            let ratio = group.ratio(e, config.stat).unwrap();
            let is_best = e == group.best(config.stat);
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
    for group in groups.iter() {
        let def = &group.data;

        writeln!(wtr, "**{}**", def.name.local)?;
        writeln!(wtr, "")?;

        writeln!(wtr, "| Parameter | Value |")?;
        writeln!(wtr, "| --------- | ----- |")?;
        writeln!(wtr, "| full name | `{}` |", def.name)?;
        writeln!(
            wtr,
            "| model | {link} |",
            link = config.url(
                &format!("`{}`", def.model),
                &format!("MODELS.md#{}", def.model)
            ),
        )?;
        if let Some(ref path) = def.regex_path {
            writeln!(
                wtr,
                "| regex-path | {link} |",
                link = config.url(
                    &format!("`{}`", path),
                    &format!("benchmarks/regexes/{}", path),
                ),
            )?;
        } else if def.regexes.is_empty() {
            writeln!(wtr, "| regex | NONE |")?;
        } else if def.regexes.len() == 1 {
            writeln!(
                wtr,
                "| regex | `````{}````` |",
                markdown_table_escape(&def.regexes[0])
            )?;
        } else {
            for (i, re) in def.regexes.iter().enumerate() {
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
            def.options.case_insensitive
        )?;
        writeln!(wtr, "| unicode | `{}` |", def.options.unicode)?;
        if let Some(ref path) = def.haystack_path {
            writeln!(
                wtr,
                "| haystack-path | {link} |",
                link = config.url(
                    &format!("`{}`", path),
                    &format!("benchmarks/haystacks/{}", path),
                ),
            )?;
        } else {
            const LIMIT: usize = 60;
            write!(wtr, "| haystack | ")?;
            let haystack = &def.haystack;
            if haystack.len() > LIMIT {
                write!(wtr, "`{} [.. snip ..]`", haystack[..LIMIT].as_bstr())?;
            } else {
                write!(wtr, "`{}`", haystack.as_bstr())?;
            }
            writeln!(wtr, " |")?;
        }
        for ec in def.count.iter() {
            writeln!(wtr, "| count(`{}`) | {} |", ec.engine, ec.count,)?;
        }

        writeln!(wtr, "")?;
        if let Some(ref analysis) = def.analysis {
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

/// Splices the given report into the given file path. This returns an error if
/// reading or writing the file fails, or if the report isn't valid UTF-8, or
/// if appropriate begin and end markers for the report could not be found.
fn splice(path: &Path, report: &[u8]) -> anyhow::Result<()> {
    let re =
        regex!(r"\n<!-- BEGIN: report -->\n((?s:.*?))<!-- END: report -->\n",);
    let src = std::fs::read_to_string(path)
        .with_context(|| path.display().to_string())?;
    let remove = match re.captures(&src) {
        None => anyhow::bail!("could not find report markers in splice file"),
        Some(caps) => caps.get(1).unwrap(),
    };
    let mut out = vec![];
    out.extend_from_slice(src[..remove.start()].as_bytes());
    out.extend_from_slice(report);
    out.extend_from_slice(&src[remove.end()..].as_bytes());
    std::fs::write(path, &out).with_context(|| path.display().to_string())?;
    Ok(())
}

/// Formats the name of something by applying various conventions used in
/// benchmark definitions.
fn nice_name(name: &str) -> String {
    let re = regex!(r"^([0-9]+-)");
    re.replace(name, "").into_owned()
}
