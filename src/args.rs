use std::{
    fmt::{Debug, Display, Write},
    str::FromStr,
};

use {
    anyhow::Context,
    lexopt::{Arg, Parser, ValueExt},
    regex_lite::Regex,
};

use crate::format::measurement::Measurement;

/// Parses the argument from the given parser as a command name, and returns
/// it. If the next arg isn't a simple valuem then this returns an error.
///
/// This also handles the case where -h/--help is given, in which case, the
/// given usage information is converted into an error and printed.
pub fn next_as_command(usage: &str, p: &mut Parser) -> anyhow::Result<String> {
    let usage = usage.trim();
    let arg = match p.next()? {
        Some(arg) => arg,
        None => anyhow::bail!("{}", usage),
    };
    let cmd = match arg {
        Arg::Value(cmd) => cmd.string()?,
        Arg::Short('h') | Arg::Long("help") => anyhow::bail!("{}", usage),
        arg => return Err(arg.unexpected().into()),
    };
    Ok(cmd)
}

/// Parses the next 'p.value()' into 'T'. Any error messages will include the
/// given flag name in them.
pub fn parse<T>(p: &mut Parser, flag_name: &'static str) -> anyhow::Result<T>
where
    T: FromStr,
    <T as FromStr>::Err: Display + Debug + Send + Sync + 'static,
{
    // This is written somewhat awkwardly and the type signature is also pretty
    // funky primarily because of the following two things: 1) the 'FromStr'
    // impls in this crate just use 'anyhow::Error' for their error type and 2)
    // 'anyhow::Error' does not impl 'std::error::Error'.
    let osv = p.value().context(flag_name)?;
    let strv = match osv.to_str() {
        Some(strv) => strv,
        None => {
            let err = lexopt::Error::NonUnicodeValue(osv.into());
            return Err(anyhow::Error::from(err).context(flag_name));
        }
    };
    let parsed = match strv.parse() {
        Err(err) => return Err(anyhow::Error::msg(err)),
        Ok(parsed) => parsed,
    };
    Ok(parsed)
}

/// This defines a flag for controlling the use of color in the output.
#[derive(Clone, Copy, Debug)]
pub enum Color {
    /// Color is only enabled when the output is a tty.
    Auto,
    /// Color is always enabled.
    Always,
    /// Color is disabled.
    Never,
}

impl Color {
    pub const USAGE: Usage = Usage::new(
        "--color <mode>",
        "One of: auto, always, never.",
        r#"
Whether to use color (default: auto).

When enabled, a modest amount of color is used to help make the output more
digestible, typically be enabling quick eye scanning. For example, when enabled
for the various benchmark comparison commands, the "best" timings are
colorized. The choices are: auto, always, never.
"#,
    );

    /// Return a possibly colorized stdout.
    #[allow(dead_code)]
    pub fn stdout(&self) -> Box<dyn termcolor::WriteColor> {
        use termcolor::{Ansi, NoColor};

        if self.should_color() {
            Box::new(Ansi::new(std::io::stdout()))
        } else {
            Box::new(NoColor::new(std::io::stdout()))
        }
    }

    /// Return a possibly colorized stderr.
    pub fn stderr(&self) -> Box<dyn termcolor::WriteColor> {
        use termcolor::{Ansi, NoColor};

        if self.should_color() {
            Box::new(Ansi::new(std::io::stderr()))
        } else {
            Box::new(NoColor::new(std::io::stderr()))
        }
    }

    /// Return a possibly colorized stdout, just like 'stdout', except the
    /// output supports elastic tabstops.
    pub fn elastic_stdout(&self) -> Box<dyn termcolor::WriteColor> {
        use {
            tabwriter::TabWriter,
            termcolor::{Ansi, NoColor},
        };

        if self.should_color() {
            Box::new(Ansi::new(TabWriter::new(std::io::stdout())))
        } else {
            Box::new(NoColor::new(TabWriter::new(std::io::stdout())))
        }
    }

    /// Return true if colors should be used. When the color choice is 'auto',
    /// this only returns true if stdout is a tty.
    pub fn should_color(&self) -> bool {
        use std::io::IsTerminal;
        match *self {
            Color::Auto => std::io::stdout().is_terminal(),
            Color::Always => true,
            Color::Never => false,
        }
    }
}

impl Default for Color {
    fn default() -> Color {
        Color::Auto
    }
}

impl std::str::FromStr for Color {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> anyhow::Result<Color> {
        let color = match s {
            "auto" => Color::Auto,
            "always" => Color::Always,
            "never" => Color::Never,
            unknown => {
                anyhow::bail!(
                    "unrecognized color config '{}', must be \
                     one of auto, always or never.",
                    unknown,
                )
            }
        };
        Ok(color)
    }
}

/// A group of filters that usually applies in most contexts. That is, it
/// contains filters for benchmark name, benchmark model and regex engine.
#[derive(Clone, Debug, Default)]
pub struct Filters {
    pub name: Filter,
    pub model: Filter,
    pub engine: Filter,
    /// When enabled, just filter out engines for which version information
    /// is not known. (Usually this means the regex engine is unavailable for
    /// one reason or another.)
    pub ignore_missing_engines: bool,
}

impl Filters {
    /// A convenience routine for checking whether the given measurement should
    /// be included with respect to the this filter.
    pub fn include(&self, m: &Measurement) -> bool {
        if !self.name.include(&m.name) {
            return false;
        }
        if !self.engine.include(&m.engine) {
            return false;
        }
        if !self.model.include(&m.model) {
            return false;
        }
        true
    }
}

/// Filter is the implementation of whitelist/blacklist rules. If there are no
/// rules, everything matches. If there's at least one whitelist rule, then you
/// need at least one whitelist rule to match to get through the filter. If
/// there are no whitelist regexes, then you can't match any of the blacklist
/// regexes.
///
/// This filter also has precedence built into that. That means that the order
/// of rules matters. So for example, if you have a whitelist regex that
/// matches AFTER a blacklist regex matches, then the input is considered to
/// have matched the filter.
#[derive(Clone, Debug, Default)]
pub struct Filter {
    rules: Vec<FilterRule>,
}

impl Filter {
    pub const USAGE_ENGINE: Usage = Usage::new(
        "-e, --engine <engine> ...",
        "Filter by including a regex engine by a name regex pattern.",
        r#"
Filter by including a regex engine by a name regex pattern.

This is just like the -f/--filter flag, except it applies to which regex
engines to include. For example, many benchmarks list a number of regex engines
that it should run with, but this filter permits specifying a smaller set of
regex engines to include.

This filter is applied to every benchmark. It is useful, for example, if you
only want to include benchmarks across two regex engines instead of all regex
engines that were specified in any given benchmark.
"#,
    );

    pub const USAGE_ENGINE_NOT: Usage = Usage::new(
        "-E, --engine-not <engine> ...",
        "Filter by excluding a regex engine by a name regex pattern.",
        r#"
Filter by excluding a regex engine by a name regex pattern.

This is just like the -f/--filter flag, except it applies to which regex
engines to include. For example, many benchmarks list a number of regex engines
that it should run with, but this filter permits specifying a smaller set of
regex engines to exclude.

This filter is applied to every benchmark. It is useful, for example, if you
want to specifically exclude a few regex engines (for example, all finite
automata engines).
"#,
    );

    pub const USAGE_BENCH: Usage = Usage::new(
        "-f, --filter <name> ...",
        "Filter by including a benchmark by a name regex pattern.",
        r#"
Filter by including a benchmark by a name regex pattern.

This flag may be given multiple times and works in concert with the
-F/--filter-not flag. The -f flag introduces a whitelist regex pattern while
-F introduces a blacklist regex pattern. Both combine to form a single filter
with the following rules. First, if there is at least one whitelist pattern,
then a benchmark must match at least one of them in order to be included.
Second, the order of the patterns matters. The last pattern that matches a
benchmark wins. For example, if '-f curated -F ruff' is given, then even though
the first pattern matches 'curated/04-ruff', since '-F ruff' came after and
is a blacklist pattern, 'curated/04-ruff' benchmarks will be excluded. Third,
regardless of how many whitelist patterns there are, a benchmark is only
included if the last pattern it matches is not a blacklist pattern.

Filter regexes are matched on the full name of the benchmark, which takes the
form '{group}/{name}'. Regexes use unanchored search. So to match the full
name, use, e.g., '^test/func/dollar-only-matches-end$'.
"#,
    );

    pub const USAGE_BENCH_NOT: Usage = Usage::new(
        "-F, --filter-not <name> ...",
        "Filter by excluding a benchmark by a name regex pattern.",
        r#"
Filter by excluding a benchmark by a name regex pattern.

This flag may be given multiple times and each time it contributes a blacklist
regex pattern to the filter for benchmark names. The rules for how this filter
works are described in more detail in the docs for the -f/--filter flag.
"#,
    );

    pub const USAGE_MODEL: Usage = Usage::new(
        "-m, --model <model> ...",
        "Filter by including a model by a name regex pattern.",
        r#"
Filter by including a model by a name regex pattern.

This is just like the -f/--filter flag, except it applies to which benchmark
models are used. For example, if you're only interested in benchmarks that
involve capture groups, then '-m capture' will automatically narrow benchmark
selection to those only with 'capture' in their model name.

The -M/--model-not flag treats the pattern as a blacklist rule.
"#,
    );

    pub const USAGE_MODEL_NOT: Usage = Usage::new(
        "-M, --model-not <model> ...",
        "Filter by excluding a model by a name regex pattern.",
        r#"
Filter by excluding a model by a name regex pattern.

This is just like the -F/--filter-not flag, except it applies to which
benchmark models are used. For example, if you don't want to see benchmarks for
the 'compile' model, then '-M compile' will excluded them.

The -m/--model flag treats the pattern as a whitelist rule.
"#,
    );

    /// Create a new filter from one whitelist regex pattern.
    ///
    /// More rules may be added, but this is a convenience routine for a simple
    /// filter.
    pub fn from_pattern(pat: &str) -> anyhow::Result<Filter> {
        let mut filter = Filter::default();
        filter.whitelist(pat)?;
        Ok(filter)
    }

    /// Add a whitelist pattern to this filter by parsing the pattern from the
    /// given arg parser.
    ///
    /// If the pattern could not be extracted from the arg parser or is not a
    /// valid regex, then this returns an error. The flag name given is used
    /// in the error message.
    pub fn arg_whitelist(
        &mut self,
        p: &mut lexopt::Parser,
        flag_name: &'static str,
    ) -> anyhow::Result<()> {
        let osval = p.value().context(flag_name)?;
        let strval = match osval.to_str() {
            Some(strval) => strval,
            None => {
                let err = lexopt::Error::NonUnicodeValue(osval.into());
                return Err(anyhow::Error::from(err).context(flag_name));
            }
        };
        self.whitelist(strval).context(flag_name)
    }

    /// Add a blacklist pattern to this filter by parsing the pattern from the
    /// given arg parser.
    ///
    /// If the pattern could not be extracted from the arg parser or is not a
    /// valid regex, then this returns an error. The flag name given is used
    /// in the error message.
    pub fn arg_blacklist(
        &mut self,
        p: &mut lexopt::Parser,
        flag_name: &'static str,
    ) -> anyhow::Result<()> {
        let osval = p.value().context(flag_name)?;
        let strval = match osval.to_str() {
            Some(strval) => strval,
            None => {
                let err = lexopt::Error::NonUnicodeValue(osval.into());
                return Err(anyhow::Error::from(err).context(flag_name));
            }
        };
        self.blacklist(strval).context(flag_name)
    }

    /// Add a whitelist pattern to this filter.
    ///
    /// If the pattern is not a valid regex, then this returns an error.
    pub fn whitelist(&mut self, pattern: &str) -> anyhow::Result<()> {
        let re =
            Regex::new(pattern).context("whitelist regex is not valid")?;
        self.rules.push(FilterRule { re, blacklist: false });
        Ok(())
    }

    /// Add a blacklist pattern to this filter.
    ///
    /// If the pattern is not a valid regex, then this returns an error.
    pub fn blacklist(&mut self, pattern: &str) -> anyhow::Result<()> {
        let re =
            Regex::new(pattern).context("blacklist regex is not valid")?;
        self.rules.push(FilterRule { re, blacklist: true });
        Ok(())
    }

    /// Return true if and only if the given subject passes this filter.
    pub fn include(&self, subject: &str) -> bool {
        // If we have no rules, then everything matches.
        if self.rules.is_empty() {
            return true;
        }
        // If we have any whitelist rules, then 'include' starts off as false,
        // as we need at least one whitelist rule in that case to match. If all
        // we have are blacklists though, then we start off with include=true,
        // and we only get excluded if one of those blacklists is matched.
        let mut include = self.rules.iter().all(|r| r.blacklist);
        for rule in &self.rules {
            if rule.re.is_match(subject) {
                include = !rule.blacklist;
            }
        }
        include
    }
}

/// A single rule in a filter, which is a combination of a regex and whether
/// it's a blacklist rule or not.
#[derive(Clone, Debug)]
pub struct FilterRule {
    re: Regex,
    blacklist: bool,
}

impl std::str::FromStr for FilterRule {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> anyhow::Result<FilterRule> {
        let (pattern, blacklist) =
            if s.starts_with('!') { (&s[1..], true) } else { (&*s, false) };
        let re = Regex::new(pattern).context("filter regex is not valid")?;
        Ok(FilterRule { re, blacklist })
    }
}

/// The choice of statistic to use. This is used in the commands for comparing
/// benchmark measurements.
#[derive(Clone, Copy, Debug)]
pub enum Stat {
    Median,
    Mad, // median absolute deviation
    Mean,
    Stddev, // standard deviation
    Min,
    Max,
}

impl Stat {
    pub const USAGE: Usage = Usage::new(
        "-s, --statistic <name>",
        "One of: median, mad, mean, stddev, min, max.",
        r#"
The aggregate statistic on which to compare (default: median).

Comparisons are only performed on the basis of a single statistic. The choices
are: median, mad (median absolute deviation), mean, stddev, min, max.
"#,
    );
}

impl Default for Stat {
    fn default() -> Stat {
        Stat::Median
    }
}

impl std::fmt::Display for Stat {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let nice = match *self {
            Stat::Median => "median",
            Stat::Mad => "mad",
            Stat::Mean => "mean",
            Stat::Stddev => "stddev",
            Stat::Min => "min",
            Stat::Max => "max",
        };
        write!(f, "{}", nice)
    }
}

impl std::str::FromStr for Stat {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> anyhow::Result<Stat> {
        let stat = match s {
            "median" => Stat::Median,
            "mad" => Stat::Mad,
            "mean" => Stat::Mean,
            "stddev" => Stat::Stddev,
            "min" => Stat::Min,
            "max" => Stat::Max,
            unknown => {
                anyhow::bail!(
                    "unrecognized statistic name '{}', must be \
                     one of median, mad, mean, stddev, min or max.",
                    unknown,
                )
            }
        };
        Ok(stat)
    }
}

/// A possibly unbounded range of speedup ratios to filter benchmarks.
///
/// This range permits one to say, "only show benchmarks where at least one
/// of the speedup ratios among the regex engines are within this range." The
/// range can be completely unbounded (include everything), or it can be
/// unbounded on either side (which corresponds to specifying either a minimum
/// or a maximum).
#[derive(Clone, Copy, Debug, Default)]
pub struct ThresholdRange {
    min: Option<Threshold>,
    max: Option<Threshold>,
}

impl ThresholdRange {
    pub const USAGE_MIN: Usage = Usage::new(
        "-t, --threshold-min <speedup-ratio>",
        "Only show benchmarks with at least this speedup ratio.",
        r#"
When set, only benchmarks that contain at least one result with at least the
speedup ratio given will be shown. In other words, this sets a lower bound on
the speedup ratio to display.

This flag may be combined with -T/--threshold-max to set an upper bound, in
which case, only benchmarks that contain at least one result whose speedup
ratio is within the range will be shown.

By default, there is no lower bound threshold enforced.

The speedup ratio for the best engine in any given set of results for a
benchmark is not considered as part of this filter. This is because the
speedup ratio for the best engine is always 1.0, and it's not usually what is
interesting to filter on.
"#,
    );

    pub const USAGE_MAX: Usage = Usage::new(
        "-T, --threshold-max <speedup-ratio>",
        "Only show benchmarks with at most this speedup ratio.",
        r#"
When set, only benchmarks that contain at least one result with at most the
speedup ratio given will be shown. In other words, this sets a upper bound on
the speedup ratio to display.

If the upper bound is less than 1.0, then it would filter everything out
because the minimum speedup ratio is 1.0.

This flag may be combined with -t/--threshold-min to set a lower bound, in
which case, only benchmarks that contain at least one result whose speedup
ratio is within the range will be shown.

By default, there is no upper bound threshold enforced.

The speedup ratio for the best engine in any given set of results for a
benchmark is not considered as part of this filter. This is because the
speedup ratio for the best engine is always 1.0, and it's not usually what is
interesting to filter on.
"#,
    );

    /// Returns true if and only if the given ratio falls within this range.
    pub fn contains(&self, ratio: f64) -> bool {
        self.min.map_or(true, |min| min.0 <= ratio)
            && self.max.map_or(true, |max| ratio <= max.0)
    }

    /// Sets the minimum of this range to the given threshold.
    pub fn set_min(&mut self, t: Threshold) {
        self.min = Some(t);
    }

    /// Sets the maximum of this range to the given threshold.
    pub fn set_max(&mut self, t: Threshold) {
        self.max = Some(t);
    }
}

/// A single threshold ratio.
///
/// Two of these make up a `ThresholdRange`.
#[derive(Clone, Copy, Debug)]
pub struct Threshold(f64);

impl std::str::FromStr for Threshold {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> anyhow::Result<Threshold> {
        let ratio = s.parse::<f64>().context("invalid threshold ratio")?;
        Ok(Threshold(ratio))
    }
}

/// The choice of units to use when representing an aggregate statistic based
/// on time.
#[derive(Clone, Copy, Debug)]
pub enum Units {
    Time,
    Throughput,
}

impl Units {
    pub const USAGE: Usage = Usage::new(
        "-u, --units <unit>",
        "One of: time, throughput.",
        r#"
The units to use in comparisons (default: thoughput).

The same units are used in all comparisons. The choices are: time or thoughput.

If any particular group of measurements are all missing throughputs (i.e.,
when their haystack length is missing or non-sensical), then absolute timings
are reported for that group instead of throughput, even when throughput was
specifically asked for.
"#,
    );
}

impl Default for Units {
    fn default() -> Units {
        Units::Throughput
    }
}

impl std::str::FromStr for Units {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> anyhow::Result<Units> {
        let stat = match s {
            "time" => Units::Time,
            "throughput" => Units::Throughput,
            unknown => {
                anyhow::bail!(
                    "unrecognized units name '{}', must be \
                     one of time or throughput.",
                    unknown,
                )
            }
        };
        Ok(stat)
    }
}

/// A type for expressing the documentation of a flag.
///
/// The `Usage::short` and `Usage::long` functions take a slice of usages and
/// format them into a human readable display. It does simple word wrapping and
/// column alignment for you.
#[derive(Clone, Debug)]
pub struct Usage {
    /// The format of the flag, for example, '-d, --bench-dir <directory>'.
    pub format: &'static str,
    /// A very short description of the flag. Should fit on one line along with
    /// the format.
    pub short: &'static str,
    /// A longer form description of the flag. May be multiple paragraphs long
    /// (but doesn't have to be).
    pub long: &'static str,
}

impl Usage {
    // We define some simpler and common flag usages right here directly.

    pub const BENCH_DIR: Usage = Usage::new(
        "-d, --dir <directory>",
        "The directory containing bench definitions",
        r#"
The directory containing benchmark definitions, haystacks and regexes.

This flag specifies the directory that contains both the benchmark definitions
and the haystacks. The benchmark definitions must be in files with a '.toml'
extension. All haystacks should be in '{{dir}}/haystacks/' and have a '.txt'
extension. Both benchmark definitions and haystacks may be in sub-directories.

The default for this value is 'benchmarks'.
"#,
    );

    pub const MAX_ITERS: Usage = Usage::new(
        "--max-iters <number>",
        "The max number of iterations to run.",
        r#"
The maximum number of iterations to run for each benchmark.

One of the difficulties of a benchmark harness is determining just how long to
run a benchmark for. We want to run it long enough that we get a decent sample,
but not too long that we are waiting forever for results. That is, there is a
point of diminishing returns.

This flag permits controlling the maximum number of iterations that a benchmark
will be executed for. In general, one should not need to change this, as it
would be better to tweak --max-time instead. However, it is exposed in case
it's useful, and in particular, you might want to increase it in certain
circumstances for an usually fast routine.
"#,
    );

    pub const MAX_WARMUP_ITERS: Usage = Usage::new(
        "--max-warmup-iters <number>",
        "The max number of warm-up iterations to run.",
        r#"
This is like --max-iters, but it applies to the number of iterations to run the
benchmark for "warm up."

Warm up refers to the part of the benchmark where it is executed and verified,
but timing samples are not collected. Warm up is used as an attempt to capture
timings that reflect average real world behavior.
"#,
    );

    pub const MAX_TIME: Usage = Usage::new(
        "--max-time <duration>",
        "The max time to run each benchmark.",
        r#"
The approximate amount of time to run a benchmark.

This harness tries to balance "benchmarks taking too long" and "benchmarks need
enough samples to be reliable" by varying the number of times each benchmark is
executed. Slower search routines (for example) get executed fewer times while
faster routines get executed more. This is done by holding invariant roughly
how long one wants each benchmark to run for. This flag sets that time.

In general, unless a benchmark is unusually fast, one should generally expect
each benchmark to take roughly this amount of time to complete.

The format for this flag is a duration specified in seconds, milliseconds,
microseconds or nanoseconds. Namely, '^[0-9]+(s|ms|us|ns)$'.
"#,
    );

    pub const MAX_WARMUP_TIME: Usage = Usage::new(
        "--max-warmup-time <duration>",
        "The max time to warm up each benchmark.",
        r#"
The approximate amount of time to warmup a benchmark.

This is like --max-time, but it controls the maximum amount of time to spending
"warming" up a benchmark. The idea of warming up a benchmark is to execute the
thing we're trying to measure for a period of time before starting the process
of collecting samples. The reason for doing this is generally to fill up any
internal caches being used to avoid extreme outliers, and even to an extent,
to give CPUs a chance to adjust their clock speeds up. The idea here is that a
"warmed" regex engine is more in line with real world use cases.

As a general rule of thumb, warmup time should be one half the benchmark time.
Indeed, if this is not given, it automatically defaults to half the benchmark
time.
"#,
    );

    /// Create a new usage from the given components.
    pub const fn new(
        format: &'static str,
        short: &'static str,
        long: &'static str,
    ) -> Usage {
        Usage { format, short, long }
    }

    /// Format a two column table from the given usages, where the first
    /// column is the format and the second column is the short description.
    pub fn short(usages: &[Usage]) -> String {
        const MIN_SPACE: usize = 2;

        let mut result = String::new();
        let max_len = match usages.iter().map(|u| u.format.len()).max() {
            None => return result,
            Some(len) => len,
        };
        for usage in usages.iter() {
            let padlen = MIN_SPACE + (max_len - usage.format.len());
            let padding = " ".repeat(padlen);
            writeln!(result, "    {}{}{}", usage.format, padding, usage.short)
                .unwrap();
        }
        result
    }

    /// Print the format of each usage and its long description below the
    /// format. This also does appropriate indentation with the assumption that
    /// it is in an OPTIONS section of a bigger usage message.
    pub fn long(usages: &[Usage]) -> String {
        let wrap_opts = textwrap::Options::new(79)
            .initial_indent("        ")
            .subsequent_indent("        ");
        let mut result = String::new();
        for (i, usage) in usages.iter().enumerate() {
            if i > 0 {
                writeln!(result, "").unwrap();
            }
            writeln!(result, "    {}", usage.format).unwrap();
            for (i, paragraph) in usage.long.trim().split("\n\n").enumerate() {
                if i > 0 {
                    result.push('\n');
                }
                let flattened = paragraph.replace("\n", " ");
                for line in textwrap::wrap(&flattened, &wrap_opts) {
                    result.push_str(&line);
                    result.push('\n');
                }
            }
        }
        result
    }
}
