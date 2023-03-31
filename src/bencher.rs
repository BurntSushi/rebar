#![allow(warnings)]

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
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
    flattened::{Engine, Flattened, Tree},
    format::{
        benchmarks::{Benchmarks, Definition, Engines, Filters},
        measurement::Measurement,
    },
    util::ShortHumanDuration,
};

// BREADCRUMBS: Add a flag to exclude regex engines from the top-level summary
// table. For example, I want to include regex/automata/meta in some of the
// benchmarks as a useful comparison point, but it probably shouldn't be listed
// in the top-level summary because it participates in too few benchmarks. It
// would just overall confuse things.

const USAGES: &[Usage] = &[
    Usage::BENCH_DIR,
    Filter::USAGE_ENGINE,
    Filter::USAGE_BENCH,
    Filter::USAGE_MODEL,
    Stat::USAGE,
    Units::USAGE,
    Usage::new(
        "--branch",
        "Branch used to run the benchmarks.",
        r#"
Branch used to run the benchmarks.

The git ref used when when collecting the timings is the branch.
If none is specified, the default testbed is used ("main").
See https://bencher.dev/docs/explanation/benchmarking for more information.
"#,
    ),
    Usage::new(
        "--testbed",
        "Testbed used to run the benchmarks.",
        r#"
Testbed used to run the benchmarks.

The testing environment used when collecting the timings is the testbed.
If none is specified, the default testbed is used ("localhost").
See https://bencher.dev/docs/explanation/benchmarking for more information.
"#,
    ),
];

const SPLICE_BEGIN: &str = "<!-- BEGIN: report -->";
const SPLICE_END: &str = "<!-- END: report -->";

fn usage() -> String {
    format!(
        "\
Save benchmark results to Bencher.

The primary input for this command is one or more CSV files that were generated
by the 'rebar measure' command. There must not be any duplicate benchmarks
among the files, or else this command will report an error.

Each regex engine is considered its own Bencher \"branch\".
The testing environment used when collecting the timings is the testbed.
Use `--testbed` to specify the testbed used to run the benchmarks.
The `rebar` benchmark path is the benchmark name.
The median latency is the metric kind.
See https://bencher.dev/docs/explanation/benchmarking for more information.

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
    let engines = flattened.engines(config.stat)?;
    let tree = Tree::new(flattened);
    bencher(&engines, config.summary_exclude.as_ref())?;
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
    ///
    /// This also ensures that every measurement has a corresponding benchmark
    /// definition.
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
            .ignore_broken_engines(true);
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

fn bencher(engines: &[Engine], exclude: Option<&Regex>) -> anyhow::Result<()> {
    let mut bmf = HashMap::new();

    let mut search_engines = engines.to_vec();
    search_engines
        .sort_by(|e1, e2| e1.geomean_search.total_cmp(&e2.geomean_search));
    bencher_run(&mut bmf, search_engines, exclude, Kind::Search)?;

    let mut compile_engines = engines.to_vec();
    compile_engines
        .sort_by(|e1, e2| e1.geomean_compile.total_cmp(&e2.geomean_compile));
    bencher_run(&mut bmf, compile_engines, exclude, Kind::Compile)?;

    let bmf_str = serde_json::to_string(&bmf)?;

    // In order to run this command you will need
    // `BENCHER_API_TOKEN` set as an environment variable.
    // If you want to avoid that for now uncomment the `--local` flag below.
    let echo_command = std::process::Command::new("echo")
        .arg(bmf_str)
        .stdout(std::process::Stdio::piped())
        .spawn()?;

    let _bencher_command = std::process::Command::new("bencher")
        .stdin(std::process::Stdio::from(
            echo_command
                .stdout
                .ok_or_else(|| anyhow::anyhow!("Failed to run"))?,
        ))
        .args([
            "run",
            "--project",
            "rebar",
            "--branch",
            // TODO: actually set this to something useful
            "main",
            "--testbed",
            // TODO: actually set this to something useful
            "localhost",
            "--adapter",
            "json",
            // "--local",
        ])
        .spawn()?;

    Ok(())
}

type BencherMetricFormat =
    HashMap<String, HashMap<String, HashMap<String, f64>>>;

enum Kind {
    Search,
    Compile,
}

impl Kind {
    fn value(&self, engine: &Engine) -> f64 {
        match self {
            Kind::Search => engine.geomean_search,
            Kind::Compile => engine.geomean_compile,
        }
    }

    fn name(&self, engine: &Engine) -> String {
        match self {
            Kind::Search => format!("{}/search", engine.name),
            Kind::Compile => format!("{}/compile", engine.name),
        }
    }
}

fn bencher_run(
    bmf: &mut BencherMetricFormat,
    engines: Vec<Engine>,
    exclude: Option<&Regex>,
    kind: Kind,
) -> anyhow::Result<()> {
    for engine in engines {
        if let Some(re) = exclude {
            if re.is_match(&engine.name) {
                continue;
            }
        }
        if engine.count_search == 0 {
            continue;
        }

        // Use the geometric mean as the value
        // TODO: create types for this or publish bencher types
        let mut metric = HashMap::with_capacity(1);
        let value = kind.value(&engine);
        if !value.is_finite() {
            continue;
        }
        metric.insert("value".into(), value);

        // Use Speed Ratio as the metric kind
        let mut geometric_mean = HashMap::with_capacity(1);
        geometric_mean.insert("speed-ratio".into(), metric);
        bmf.insert(kind.name(&engine), geometric_mean);
    }

    Ok(())
}
