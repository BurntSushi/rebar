use std::{
    io::{BufReader, Read},
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

use {anyhow::Context, bstr::ByteSlice};

use crate::{
    args::{self, Filter, Filters, Usage},
    format::{
        benchmarks::{Benchmarks, Definition, Engine},
        measurement::{Aggregate, AggregateTimes, Measurement},
    },
    util::{self, ShortHumanDuration},
};

const MIN_TIMEOUT: Duration = Duration::from_secs(10);

const USAGES: &[Usage] = &[
    Usage::BENCH_DIR,
    Filter::USAGE_ENGINE,
    Filter::USAGE_ENGINE_NOT,
    Filter::USAGE_BENCH,
    Filter::USAGE_BENCH_NOT,
    Usage::new(
        "-i, --ignore-missing-engines",
        "Silently suppress missing regex engines.",
        r#"
This silently suppresses "missing" regex engines. "Missing" in this context
means a regex engine whose version information could not be found. This might
happen due to an improperly configured 'engines.toml', or it might just be
because the regex engine isn't available for one reason or another. Usually
that's because it failed to build, which should result in an error appearing
during 'rebar build'.

This option is useful for when you just want to implicitly filter out any regex
engines that cannot be benchmarked. Otherwise, an attempt will still be made
and it will result in reporting a measurement error.
"#,
    ),
    Usage::new(
        "--list",
        "List benchmarks, but don't run them.",
        r#"
List benchmarks to run, but don't run them.

This command does all of the work to collect benchmarks, haystacks, filter them
and validate them. But it does not actually run the benchmarks. Instead, it
prints every benchmark that will be executed. This is useful for seeing what
work will be done without actually doing it.
"#,
    ),
    Usage::MAX_ITERS,
    Usage::MAX_WARMUP_ITERS,
    Usage::MAX_TIME,
    Usage::MAX_WARMUP_TIME,
    Filter::USAGE_MODEL,
    Filter::USAGE_MODEL_NOT,
    Usage::new(
        "-t/--test",
        "Alias for --verify --verbose.",
        r#"
An alias for --verify --verbose. The combination of --verify and --verbose is
quite common for being able to confirm that benchmarks run successfully and
being able to see the full error messages if anything goes wrong.
"#,
    ),
    Usage::new(
        "--timeout <duration>",
        "Kill a benchmark if it exceeds this.",
        r#"
Attempts to kill a benchmark if it exceeds this duration.

This is set by default to twice the combined time of --max-time and
--max-warmup-time.

This is useful to keep long running benchmarks in check. In general, there
should be no benchmarks that trip this timeout regularly, but the timeout is
still useful because different environments might execute much more slowly than
one might expect.
"#,
    ),
    Usage::new(
        "--verbose",
        "Print extra information in some cases.",
        r#"
Print extra information where possible.

Where possible, this prints extra information. e.g., When using --verify, this
will print each benchmark that is being tested as it happens, as a way to see
progress.
"#,
    ),
    Usage::new(
        "--verify",
        "Verify that benchmarks run correctly.",
        r#"
Verify that all selected benchmarks run successfully.

This checks that all selected benchmarks can run through at least one iteration
without reporting an error or an incorrect answer. This can be useful for
quickly debugging a new benchmark or regex engine where the answers aren't
lining up.

This collects all errors reported and prints them. If no errors occurred, then
this prints nothing and exits successfully.
"#,
    ),
];

fn usage_short() -> String {
    format!(
        "\
Run benchmarks and write measurements.

USAGE:
    rebar measure [OPTIONS]

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
Run benchmarks and write measurements.

To compare benchmark results use 'rebar diff' for comparing results across time
for each regex engine, and 'rebar cmp' for comparing results between regex
engines.

USAGE:
    rebar measure [OPTIONS]

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
    // Parse everything and load what we need.
    let config = Config::parse(p)?;
    let benchmarks = config.read_benchmarks()?;

    // Collect all of the benchmarks we will run. Each benchmark definition can
    // spawn multiple benchmarks; one for each regex engine specified in the
    // definition.
    let mut exec_benchmarks = vec![];
    for def in benchmarks.defs.iter() {
        for result in ExecBenchmarkIter::new(&config.bench_config, def) {
            let b = result?;
            // While we did run the engine filter above when we initially
            // collected our benchmarks, we run it again because the filter
            // above only excludes benchmark definitions that have no matching
            // engines at all. But we might still run a subset of the engines
            // in a particular benchmark definition. So why do we run it above?
            // Well, this way, we avoid loading haystacks into memory that will
            // never be used.
            if !config.filters.engine.include(&b.engine.name) {
                continue;
            }
            exec_benchmarks.push(b);
        }
    }
    // If we just want to list which benchmarks we'll run, spit that out.
    if config.list {
        let mut wtr = csv::Writer::from_writer(std::io::stdout());
        for b in exec_benchmarks.iter() {
            wtr.write_record(&[
                b.def.name.to_string(),
                b.def.model.to_string(),
                b.engine.name.clone(),
                b.engine.version.clone(),
            ])?;
        }
        wtr.flush()?;
        return Ok(());
    }
    // Or if we just want to check that every benchmark runs correctly, do
    // that. We spit out any error we find.
    if config.verify {
        let mut errored = false;
        let mut wtr = csv::Writer::from_writer(std::io::stdout());
        for b in exec_benchmarks.iter() {
            let agg = b.aggregate(b.verifier().collect(config.verbose));
            if let Some(err) = agg.err {
                errored = true;
                wtr.write_record(&[
                    b.def.name.to_string(),
                    b.def.model.to_string(),
                    b.engine.name.clone(),
                    b.engine.version.clone(),
                    format!("{:#}", err),
                ])?;
            } else if config.verbose {
                wtr.write_record(&[
                    b.def.name.to_string(),
                    b.def.model.to_string(),
                    b.engine.name.clone(),
                    b.engine.version.clone(),
                    "OK".to_string(),
                ])?;
            }
            wtr.flush()?;
        }
        anyhow::ensure!(!errored, "some benchmarks failed");
        return Ok(());
    }
    // Run our benchmarks and emit the results of each as a single CSV record.
    let mut wtr = csv::Writer::from_writer(std::io::stdout());
    for b in exec_benchmarks.iter() {
        // Run the benchmark, collect the samples and turn the samples into a
        // collection of various aggregate statistics (mean+/-stddev, median,
        // min, max).
        let agg = b.aggregate(b.collect(config.verbose));
        // Our aggregate is initially captured in terms of how long it takes to
        // execute each iteration of the benchmark. But for searching, this is
        // not particularly intuitive. Instead, we convert strict timings into
        // throughputs, which give a much better idea of how fast something is
        // by relating it to how much can be searched in a single second.
        //
        // Literally every regex benchmark I've looked at reports measurements
        // as raw timings. Like, who the heck cares if a regex search completes
        // in 500ns? What does that mean? It's much clearer to say 500 MB/s.
        // I guess people consistently misunderstand that benchmarks are
        // fundamentally about communication first.
        //
        // Using throughputs doesn't quite make sense for the 'compile'
        // benchmarks, and indeed, we set it up so that we don't capture any
        // haystack length for them. This causes the units to be in absolute
        // time by default.
        wtr.serialize(agg)?;
        // Flush every record once we have it so that users can see that
        // progress is being made.
        wtr.flush()?;
    }
    Ok(())
}

/// The CLI arguments parsed from the 'measure' sub-command.
#[derive(Clone, Debug, Default)]
struct Config {
    /// The directory to find benchmark definitions and haystacks.
    dir: PathBuf,
    /// The benchmark name, model and regex engine filters.
    filters: Filters,
    /// Various parameters to control how ever benchmark is executed.
    bench_config: ExecBenchmarkConfig,
    /// Whether to just list the benchmarks that will be executed and
    /// then quit. This also tests that all of the benchmark data can be
    /// deserialized.
    list: bool,
    /// Whether to just verify all of the benchmarks without collecting any
    /// measurements.
    verify: bool,
    /// When enabled, print extra stuff where appropriate.
    verbose: bool,
}

impl Config {
    /// Parse 'measure' args from the given CLI parser.
    fn parse(p: &mut lexopt::Parser) -> anyhow::Result<Config> {
        use lexopt::Arg;

        let mut c = Config::default();
        c.dir = PathBuf::from("benchmarks");
        while let Some(arg) = p.next()? {
            match arg {
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
                Arg::Short('i') | Arg::Long("ignore-missing-engines") => {
                    c.filters.ignore_missing_engines = true;
                }
                Arg::Long("list") => {
                    c.list = true;
                }
                Arg::Long("max-iters") => {
                    c.bench_config.max_iters = args::parse(p, "--max-iters")?;
                }
                Arg::Long("max-warmup-iters") => {
                    c.bench_config.max_warmup_iters =
                        args::parse(p, "--max-warmup-iters")?;
                }
                Arg::Long("max-time") => {
                    let hdur =
                        args::parse::<ShortHumanDuration>(p, "--max-time")?;
                    c.bench_config.max_time = Duration::from(hdur);
                }
                Arg::Long("max-warmup-time") => {
                    let hdur = args::parse::<ShortHumanDuration>(
                        p,
                        "--max-warmup-time",
                    )?;
                    c.bench_config.max_warmup_time = Duration::from(hdur);
                }
                Arg::Short('m') | Arg::Long("model") => {
                    c.filters.model.arg_whitelist(p, "-m/--model")?;
                }
                Arg::Short('M') | Arg::Long("model-not") => {
                    c.filters.model.arg_blacklist(p, "-M/--model-not")?;
                }
                Arg::Short('t') | Arg::Long("test") => {
                    c.verbose = true;
                    c.verify = true;
                }
                Arg::Long("timeout") => {
                    let hdur =
                        args::parse::<ShortHumanDuration>(p, "--timeout")?;
                    c.bench_config.timeout = Duration::from(hdur);
                }
                Arg::Long("verbose") => {
                    c.verbose = true;
                }
                Arg::Long("verify") => {
                    c.verify = true;
                }
                _ => return Err(arg.unexpected().into()),
            }
        }
        Ok(c)
    }

    /// Read and parse benchmark definitions from TOML files in the --dir
    /// directory.
    fn read_benchmarks(&self) -> anyhow::Result<Benchmarks> {
        Benchmarks::from_dir(&self.dir, &self.filters)
    }
}

/// The configuration for a benchmark. This is overridable via the CLI, and can
/// be useful on a case-by-case basis. In effect, it controls how benchmarks
/// are executed and generally permits explicitly configuring how long you
/// want to wait for benchmarks to run. Nobody wants to wait a long time, but
/// you kind of need to wait a little bit or else benchmark results tend to be
/// quite noisy.
#[derive(Clone, Debug)]
struct ExecBenchmarkConfig {
    /// The maximum number of samples to collect.
    max_iters: u64,
    /// The maximum number of times to execute the benchmark before collecting
    /// samples.
    max_warmup_iters: u64,
    /// The approximate amount of time the benchmark should run. The idea here
    /// is to collect as many samples as possible, up to the max and only for
    /// as long as we are in our time budget.
    ///
    /// It'd be nice if we could just collect the same number of samples for
    /// every benchmark, but this is in practice basically impossible when your
    /// benchmarks include things that are blindingly fast like 'memmem' and
    /// things that are tortoise slow, like the Pike VM.
    max_time: Duration,
    /// Like max benchmark time, but for warmup time. As a general rule, it's
    /// usually good to have this be about half the benchmark time.
    max_warmup_time: Duration,
    /// After this amount of time has passed, the benchmark runner is
    /// unceremoniously killed and measurement reporting for that benchmark
    /// fails.
    timeout: Duration,
}

impl Default for ExecBenchmarkConfig {
    fn default() -> ExecBenchmarkConfig {
        let max_time = Duration::from_millis(3000);
        let max_warmup_time = max_time / 2;
        let timeout =
            std::cmp::max(MIN_TIMEOUT, 2 * (max_time + max_warmup_time));
        ExecBenchmarkConfig {
            max_warmup_iters: 1_000_000,
            max_iters: 1_000_000,
            max_time,
            max_warmup_time,
            timeout,
        }
    }
}

/// An iterator over all benchmarks from a benchmark definition.
///
/// The lifetime `'d` refers to the benchmark definition from which to generate
/// benchmarks.
#[derive(Debug)]
struct ExecBenchmarkIter<'a> {
    config: &'a ExecBenchmarkConfig,
    def: &'a Definition,
    it: std::slice::Iter<'a, Engine>,
}

impl<'a> ExecBenchmarkIter<'a> {
    fn new(
        config: &'a ExecBenchmarkConfig,
        def: &'a Definition,
    ) -> ExecBenchmarkIter<'a> {
        let it = def.engines.iter();
        ExecBenchmarkIter { config, def, it }
    }
}

impl<'b> Iterator for ExecBenchmarkIter<'b> {
    type Item = anyhow::Result<ExecBenchmark>;

    fn next(&mut self) -> Option<anyhow::Result<ExecBenchmark>> {
        let engine = self.it.next()?.clone();
        Some(Ok(ExecBenchmark {
            config: self.config.clone(),
            def: self.def.clone(),
            engine,
        }))
    }
}

/// A single benchmark that can be executed in order to collect timing samples.
/// Each sample corresponds to a single run of a single regex engine on a
/// particular haystack.
#[derive(Clone, Debug)]
struct ExecBenchmark {
    /// The config, given from the command line.
    config: ExecBenchmarkConfig,
    /// The definition, taken from TOML data.
    def: Definition,
    /// The name of the regex engine to execute. This is guaranteed to match
    /// one of the values in 'def.engines'.
    engine: Engine,
}

impl ExecBenchmark {
    /// Run and collect the results of this benchmark.
    ///
    /// This interrogates the benchmark type and runs the corresponding
    /// benchmark function to produce results.
    fn collect(&self, verbose: bool) -> anyhow::Result<Results> {
        use std::process::Stdio;

        // If we don't know the version of the engine then we absolutely refuse
        // to collect measurements. Results should always include the version
        // measured, otherwise we're doing a disservice to folks looking at the
        // results.
        //
        // If you don't want to see these errors, then pass
        // --ignore-missing-engines.
        anyhow::ensure!(
            !self.engine.is_missing_version(),
            "invalid version for regex engine",
        );

        // This is kind of a brutal function, and I was tempted to split it
        // up into more pieces, but it's not totally clear if it's worth doing
        // or even what those pieces would be. The main complexity here is
        // that we need to pipe something to stdin, we need to read stdout
        // and we might read stderr when verbose mode is disabled, or we might
        // just let stderr pass through when verbose mode is enabled. There
        // are also TONS of a failure points, and for that reason, we try to
        // give descriptive error messages where we can.

        let mut cmd =
            self.engine.run.command().context("failed to build command")?;
        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(if verbose { Stdio::inherit() } else { Stdio::piped() });
        log::debug!(
            "running command: \
             \"{}\" \"klv\" \"{}\" \
             \"--max-iters\" \"{}\" \
             \"--max-warmup-iters\" \"{}\" \
             \"--max-time\" \"{}\" \
             \"--max-warmup-time\" \"{}\" \
             | {:?}",
            util::current_exe()?,
            self.def.name,
            self.config.max_iters,
            self.config.max_warmup_iters,
            self.config.max_time.as_nanos(),
            self.config.max_warmup_time.as_nanos(),
            cmd,
        );
        let spawn_start = Instant::now();
        let mut child = cmd.spawn().context("failed to spawn process")?;

        let handle_stdin = {
            let klvbench = klv::Benchmark {
                name: self.def.name.as_str().to_string(),
                model: self.def.model.clone(),
                regex: klv::Regex {
                    patterns: self
                        .def
                        .regexes
                        .iter()
                        .map(|p| p.to_string())
                        .collect(),
                    case_insensitive: self.def.options.case_insensitive,
                    unicode: self.def.options.unicode,
                },
                haystack: Arc::clone(&self.def.haystack),
                max_iters: self.config.max_iters,
                max_warmup_iters: self.config.max_warmup_iters,
                max_time: self.config.max_time,
                max_warmup_time: self.config.max_warmup_time,
            };
            let mut stdin = child.stdin.take().unwrap();
            std::thread::spawn(move || -> anyhow::Result<()> {
                klvbench
                    .write(&mut stdin)
                    .context("failed to write KLV data to stdin")?;
                Ok(())
            })
        };
        let handle_stdout = {
            let mut stdout = child.stdout.take().unwrap();
            std::thread::spawn(move || -> anyhow::Result<Vec<u8>> {
                let mut buf = vec![];
                stdout
                    .read_to_end(&mut buf)
                    .context("failed to read stdout")?;
                Ok(buf)
            })
        };
        // When verbose mode is enabled, we let stderr inherit from the rebar
        // process so that it just pipes right through. As a result, if the
        // benchmark fails, since we didn't capture stderr, we just write
        // a generic error message. But when verbose mode is disabled, we
        // capture stderr like we do stdout, and use stderr to improve the
        // error reporting in the benchmark results.
        let handle_stderr = if verbose {
            None
        } else {
            let mut stderr = BufReader::new(child.stderr.take().unwrap());
            Some(std::thread::spawn(move || -> anyhow::Result<Vec<u8>> {
                let mut buf = vec![];
                stderr
                    .read_to_end(&mut buf)
                    .context("failed to read stderr")?;
                Ok(buf)
            }))
        };
        // Sometimes benchmarks might take a long time, so we periodically
        // ping the sub-process to see if it's done. If it hasn't finished
        // after a period of time, we kill the process and report a measurement
        // failure.
        //
        // In general, we try to avoid defining such benchmarks, but maybe
        // different environments execute things more slowly. This is also
        // useful during experimentation, where you might not know how long a
        // regex will take.
        let status = loop {
            let maybe_status =
                child.try_wait().context("failed to reap process")?;
            if let Some(status) = maybe_status {
                break status;
            }
            if spawn_start.elapsed() > self.config.timeout {
                log::debug!(
                    "benchmark time exceeded {:?}, killing process",
                    self.config.timeout,
                );
                if let Err(err) = child.kill() {
                    log::debug!(
                        "failed to kill command {:?} because {}",
                        cmd,
                        err,
                    );
                } else {
                    log::debug!("successfully killed {:?}", cmd);
                    log::debug!("reaping...");
                    match child.wait() {
                        Ok(status) => {
                            log::debug!(
                                "reap successful, exit status: {:?}",
                                status
                            );
                        }
                        Err(err) => {
                            log::debug!("reap failed: {}", err);
                        }
                    }
                }
                anyhow::bail!("timeout: exceeded {:?}", self.config.timeout);
            }
            std::thread::sleep(Duration::from_millis(50));
        };
        // We wait to handle any errors from writing to stdin until we've dealt
        // with stderr, since stderr is likely to contain the actual error that
        // occurred. That is, if writing to stdin failed, then it's likely
        // because the process itself failed and the pipe was severed. So the
        // underlying cause is almost certainly on stderr. Still, we join all
        // of the threads to make sure they've completed.
        let result_stdin = handle_stdin.join().unwrap();
        let result_stdout = handle_stdout.join().unwrap();
        let stderr = match handle_stderr {
            None => vec![],
            Some(handle) => handle.join().unwrap()?,
        };
        if !status.success() {
            if verbose {
                anyhow::bail!(
                    "failed to run command for '{}'",
                    self.engine.name
                );
            }
            let last = match stderr.lines().last() {
                Some(last) => last,
                None => {
                    anyhow::bail!(
                        "failed to run command for '{}' but stderr was empty",
                        self.engine.name,
                    );
                }
            };
            anyhow::bail!(
                "failed to run command for '{}', last line of stderr is: {}",
                self.engine.name,
                last.as_bstr(),
            );
        }
        let stdout = result_stdout?;
        result_stdin?;

        let expected_count = self.def.count(&self.engine.name)?;
        let mut results = Results::new(self);
        for line in stdout.lines() {
            let (field1, field2) = match line.split_once_str(",") {
                Some((f1, f2)) => (f1, f2),
                None => anyhow::bail!(
                    "when running '{}', got invalid sample format {:?}",
                    self.engine.name,
                    line.as_bstr()
                ),
            };
            let s1 = field1.to_str().with_context(|| {
                format!(
                    "failed to parse duration field {:?} as UTF-8",
                    field1.as_bstr()
                )
            })?;
            let s2 = field2.to_str().with_context(|| {
                format!(
                    "failed to parse count field {:?} as UTF-8",
                    field2.as_bstr()
                )
            })?;
            let nanos = s1.parse::<u64>().with_context(|| {
                format!("failed to parse duration field {:?} as u64", s1)
            })?;
            // If we get a measurement of 0 nanoseconds, then that winds up
            // being pretty meaningless. So we "round up" to 1. Basically, we
            // just give up trying to measure anything that reliably takes less
            // than 1 nanosecond.
            let duration =
                Duration::from_nanos(if nanos == 0 { 1 } else { nanos });
            let count = s2.parse::<u64>().with_context(|| {
                format!("failed to parse count field {:?} as u64", s2)
            })?;
            anyhow::ensure!(
                count == expected_count,
                "count mismatch, expected {}, got {}",
                expected_count,
                count,
            );
            results.samples.push(duration);
        }
        results.total = spawn_start.elapsed();
        Ok(results)
    }

    /// Turn the given results collected from running this benchmark into
    /// a single set of aggregate statistics describing the samples in the
    /// results.
    fn aggregate(&self, result: anyhow::Result<Results>) -> Measurement {
        match result {
            Ok(results) => results.to_measurement(),
            Err(err) => self.measurement_error(format!("{:#}", err)),
        }
    }

    /// Create a new "error" aggregate from this benchmark with the given
    /// error message. This is useful in cases where the benchmark couldn't
    /// run or there was some other discrepancy. Folding the error into the
    /// aggregate value itself avoids recording the error "out of band" and
    /// also avoids silently squashing it.
    fn measurement_error(&self, err: String) -> Measurement {
        Measurement {
            name: self.def.name.to_string(),
            model: self.def.model.to_string(),
            rebar_version: util::version(),
            engine: self.engine.name.clone(),
            engine_version: self.engine.version.clone(),
            err: Some(err),
            ..Measurement::default()
        }
    }

    /// This creates a new `Benchmark` that is suitable purely for
    /// verification. Namely, it modifies any config necessary to ensure that
    /// the benchmark will run only one iteration and report the result.
    fn verifier(&self) -> ExecBenchmark {
        let config = ExecBenchmarkConfig {
            max_iters: 1,
            max_warmup_iters: 0,
            max_time: Duration::ZERO,
            max_warmup_time: Duration::ZERO,
            timeout: self.config.timeout,
        };
        ExecBenchmark {
            config,
            def: self.def.clone(),
            engine: self.engine.clone(),
        }
    }
}

/// The raw results generated by running a benchmark.
#[derive(Clone, Debug)]
struct Results {
    /// The benchmark that was executed.
    benchmark: ExecBenchmark,
    /// The total amount of time that the benchmark ran for.
    total: Duration,
    /// The individual timing samples collected from the benchmark. Each sample
    /// represents the time it takes for a single run of the thing being
    /// measured. This does not include warmup iterations.
    samples: Vec<Duration>,
}

impl Results {
    /// Create a new empty set of results for the given benchmark.
    fn new(b: &ExecBenchmark) -> Results {
        Results {
            benchmark: b.clone(),
            total: Duration::default(),
            samples: vec![],
        }
    }

    /// Convert these results into aggregate statistical values. If there are
    /// no samples, then an "error" measurement is returned.
    fn to_measurement(&self) -> Measurement {
        let mut samples = vec![];
        for &dur in self.samples.iter() {
            samples.push(dur.as_secs_f64());
        }
        // It's not quite clear how this could happen, but it's definitely
        // an error. This also makes some unwraps below OK, because we can
        // assume that 'timings' is non-empty.
        if samples.is_empty() {
            let err = "no samples or errors recorded".to_string();
            return self.benchmark.measurement_error(err);
        }
        // We have no NaNs, so this is fine.
        samples.sort_unstable_by(|x, y| x.partial_cmp(y).unwrap());
        let haystack_len = match &*self.benchmark.def.model {
            // This is somewhat unfortunate. This is, I believe, the *only*
            // place inside of rebar that cares at all about a specific model
            // string. It would be nice to remove this, but it seems like we'd
            // need to add another layer of configuration to do so? That's a
            // pretty big bummer...
            "compile" | "regex-redux" => None,
            _ => {
                // We don't expect to have haystacks bigger than 2**64.
                u64::try_from(self.benchmark.def.haystack.len()).ok()
            }
        };
        let times = AggregateTimes {
            // OK because timings.len() > 0
            median: Duration::from_secs_f64(median(&samples).unwrap()),
            // OK because timings.len() > 0
            mad: Duration::from_secs_f64(mad(&samples).unwrap()),
            // OK because timings.len() > 0
            mean: Duration::from_secs_f64(mean(&samples).unwrap()),
            // OK because timings.len() > 0
            stddev: Duration::from_secs_f64(stddev(&samples).unwrap()),
            // OK because timings.len() > 0
            min: Duration::from_secs_f64(min(&samples).unwrap()),
            // OK because timings.len() > 0
            max: Duration::from_secs_f64(max(&samples).unwrap()),
        };
        Measurement {
            name: self.benchmark.def.name.to_string(),
            model: self.benchmark.def.model.to_string(),
            rebar_version: util::version(),
            engine: self.benchmark.engine.name.clone(),
            engine_version: self.benchmark.engine.version.clone(),
            err: None,
            // We don't expect iterations to exceed 2**64.
            iters: u64::try_from(samples.len()).unwrap(),
            total: self.total,
            aggregate: Aggregate::new(times, haystack_len),
        }
    }
}

fn mean(xs: &[f64]) -> Option<f64> {
    if xs.is_empty() {
        None
    } else {
        let sum: f64 = xs.iter().sum();
        Some(sum / (xs.len() as f64))
    }
}

fn stddev(xs: &[f64]) -> Option<f64> {
    let len = xs.len() as f64;
    let mean = mean(xs)?;
    let mut deviation_sum_squared = 0.0;
    for &x in xs.iter() {
        deviation_sum_squared += (x - mean).powi(2);
    }
    Some((deviation_sum_squared / len).sqrt())
}

fn median(xs: &[f64]) -> Option<f64> {
    if xs.is_empty() {
        None
    } else if xs.len() % 2 == 1 {
        // Works because integer division rounds down
        Some(xs[xs.len() / 2])
    } else {
        let second = xs.len() / 2;
        let first = second - 1;
        mean(&[xs[first], xs[second]])
    }
}

fn mad(xs: &[f64]) -> Option<f64> {
    let xmed = median(xs)?;
    let mut devs = xs.iter().map(|x| (x - xmed).abs()).collect::<Vec<f64>>();
    devs.sort_unstable_by(|x, y| x.partial_cmp(y).unwrap());
    median(&devs)
}

fn min(xs: &[f64]) -> Option<f64> {
    let mut it = xs.iter().copied();
    let mut min = it.next()?;
    for x in it {
        if x < min {
            min = x;
        }
    }
    Some(min)
}

fn max(xs: &[f64]) -> Option<f64> {
    let mut it = xs.iter().copied();
    let mut max = it.next()?;
    for x in it {
        if x > max {
            max = x;
        }
    }
    Some(max)
}
