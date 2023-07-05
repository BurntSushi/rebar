# Benchmark Definition

This document describes the format for benchmark definitions. In general, this
regex barometer takes the approach of defining benchmarks as separate from
code. Each benchmark definition answers the following questions:

1. What kind of benchmark is it? Does it just count all matches? Or all
matching lines? Or all matching capturing groups?
2. What is the group and name of the benchmark?
3. Which regexes are being measured?
4. How are the regexes compiled?
5. What will the regexes search?
6. What is the expected result of that search?
7. Which regex engine implementations will be measured?

The `rebar` harness command takes answers to these questions, executes the
benchmark and reports the results.

## Overview

Benchmarks are defined via a directory hierarchy. For a directory `bench_dir`,
`rebar` expects it to look like this:

* `{bench_dir}/engines.toml` is a TOML file that specifies a list of engines
that should be benchmarked. This list contains the full set of regex engine
names that may be used in benchmark definitions.
* `{bench_dir}/definitions` contains TOML files, where each file corresponds to
a group of benchmark definitions. Each definition specifies the regexes to run,
the haystack to search, the count to expect and more. The basename of each TOML
file (without the `.toml` extension) must match the regex `^[-A-Za-z0-9]+$`.
Files other than TOML files are ignored.
* `{bench_dir}/haystacks` contains files that serve as haystacks. Haystacks
may be defined directly inside of TOML files (as explained below), but it's
often more convenient to put larger haystacks in their own file. A haystack
at `{bench_dir}/haystacks/foo/bar/quux.txt` can be referenced in a benchmark
definition TOML file via the path `foo/bar/quux.txt`. There are no restrictions
placed on the contents of a haystack file. Files in this directory are only
read if they are referenced from a TOML file. Otherwise they are ignored.
* `{bench_dir}/regexes` contains files that provide regex patterns. While not
as commonly used as files containing haystacks, some regexes (like
dictionaries) can be quite large and thus may benefit from being in a separate
file. As with haystacks, a regex at `{bench_dir}/regexes/foo/bar/quux` can be
referenced in a TOML file via the path `foo/bar/quux`. The contents of a regex
file must be valid UTF-8. As with haystacks, files in this directory are only
read if they are referenced from a TOML file. Otherwise they are ignored.
* All other files are ignored.

By default, `rebar` assumes that `bench_dir` is set to `./benchmarks`. This
means that running `rebar` in the root of this repository will do the right
thing by default. The directory may be overridden via the `-d/--dir` flag.

## Engine TOML Format

The `{bench_dir}/engines.toml` file defines a list of regex engines that rebar
knows how to execute. Every regex engine is composed of the following parts:

* A name, which must be unique.
* A way to retrieve the version of the regex engine. This also must serve as
a receipt that the regex engine is available to run. For example, if the
version is unavailable, then the `-i/--ignore-missing-engines` rebar flag will
behave as if those engines don't exist. (And thus reduce noisy error output.)
* A way to run the regex engine via a process.
* Optionally, a way to build the regex engine. For example, Go's regex engine
is built via `go build`. (Regex engines that are bundled with `rebar` do not
have any build steps, but most others do.)
* Optionally, a way to clean up any artifacts produced by the build step.

A new engine can be added by writing `[[engine]]`. Each engine supports the
following keys:

* `name` - The name of the engine. This must be unique.
* `cwd` - An optional key that specifies the working directory in which all
commands for this engine are run. This can be overridden in each command.
The `cwd` is interpreted relative to `{bench_dir}`, i.e., the directory
containing the `engines.toml` file. If omitted, the working directory defaults
to `{bench_dir}`. (Which is `./benchmarks/` by default.)
* `version` - A TOML table specifying how to get the version of this regex
engine.
* `run` - A TOML table specifying a command to run the regex engine.
* `dependency` - An array of TOML tables specifying commands to run to check
that the necessary dependencies are installed. These are present strictly to
improve failure modes. Build tools often assume the existence of certain things
that are installed and output very strange and difficult to diagnose error
messages when they aren't. These commands are meant to help make those sorts
of errors easier to comprehend by catching them earlier.
* `build` - An array of TOML tables specifying commands to run to build the
regex engine.
* `clean` - An array of TOML tables specifying commands to run to clean the
artifacts produced by building a regex engine.

The command table has the following keys:

* `cwd` - Sets the working directory in which this command is run, relative to
`{bench_dir}`. If it's absent, then the `cwd` set on the engine is used. If
that's absent, then it's set to `{bench_dir}`.
* `bin` - A string corresponding to the binary name of the program. Note that
because of platform idiosyncracies, when `cwd` is set (either here or on the
engine) and `bin` contains a `/`, then it is assumed to be relative path. The
final bin used is then `cwd(rebar)/cwd(engine or command)/bin`. If the binary
name contains no slashes, then it is used as-is and likely relies on the value
of your environment's `PATH` to be resolved.
* `args` - An optional array of arguments to call `bin` with.
* `envs` - An optional array of environment variables. Each environment
variable is itself a table, with string keys `name` and `value`.

The `version` table is a combination of the command table described above and
the following keys:

* `file` - In lieu of a command, the version string may be read from a file.
Note that if this is used, the file should be generated as part of the `build`
process and removed as part of the `clean` process. This is because the version
_should_ serve as a recept that the regex engine is available to read.
* `regex` - An optional regular expression used to capture the version from the
output of the command that was run or the `file` that was specified. The regex
must have a capturing group with name `version`.

The `dependency` table is a combination of the command table described above
and the following keys:

* `regex` - An optional regular expression used to search the output of
the dependency command. If the regex search fails, then the dependency is
considered unavailable and the regex engine won't build.

Here's a quick example that shows how Python's `regex` engine is defined (this
is the third party `regex` module and not the standard library `re` module):

```toml
[[engine]]
  name = "python/regex"
  cwd = "../engines/python"
  [engine.version]
    bin = "ve/bin/pip"
    args = ["show", "regex"]
    regex = '(?m)^Version: (?P<version>.+)$'
  [engine.run]
    bin = "ve/bin/python"
    args = ["main.py", "regex"]
  [[engine.dependency]]
    bin = "python"
    args = ["--version"]
    regex = '(?m)^Python 3\.'
  [[engine.dependency]]
    bin = "virtualenv"
    args = ["--version"]
    regex = '(?m)^virtualenv\s+'
  [[engine.build]]
    bin = "virtualenv"
    args = ["ve"]
  [[engine.build]]
    bin = "ve/bin/pip"
    args = ["install", "regex"]
  [[engine.clean]]
    bin = "rm"
    args = ["-rf", "./ve"]
```

## Benchmark definition TOML Format

Each benchmark definition TOML file corresponds to one group containing zero or
more benchmark definitions. A benchmark definition can be introduced by adding
to the `bench` array, and each entry in that array supports the following
fields. We only include a short description for each field here. More details
about each follow below.

* `model` - The model used for the benchmark.
* `name` - The name of the benchmark within a particular group.
* `regex` - The regex pattern to measure.
* `case-insensitive` - Whether to enable case insensitive searching.
* `unicode` - Whether to enable Unicode support in the regex pattern.
* `haystack` - The data to search.
* `count` - The expected number of matches.
* `engines` - An array of names corresponding to the regex engines to
measure for this benchmark.

Here's a quick example that doesn't demonstrate everything, but shows how a
simple "count all matches" benchmark is defined:

```toml
[[bench]]
model = "count"
name = "before-after-holmes"
regex = '\w+\s+Holmes\s+\w+'
haystack = { path = "sherlock.txt" }
count = 137
engines = [
  'regex/api',
  're2/api',
  'pcre2/api/jit',
  'pcre2/api/nojit',
]
```

### `model`

The `model` field tells `rebar` which benchmarking model to use. Each benchmark
is meant to *model* a real world use case of a regex. Measuring multiple
models is important for a barometer, because the model can have an impact on
performance. For example, `count` benchmarks tend to be suited for searching
a single large haystack, where as a `grep` benchmark will search many short
haystacks. Sometimes, different models measure completely different aspects
of a regex engine that range in importance depending on context. For example,
`compile` benchmarks give a sense of how long a regex engine takes to build a
regex and does not measure search speed at all.

In general, every benchmark model computes some kind of count that is compared
with the count in the benchmark definition. If the result differs, then the
measurement for that regex engine fails and is not collected. This ensures that
every regex engine produces the expected result.

The possible values for the `model` field are:

* `compile` - Measures the compilation time of a regex.
* `count` - Measures a count of all matches in a haystack.
* `count-spans` - Measures a sum of all match lengths in a haystack.
* `count-captures` - Measures a count of all matching capturing groups in a
haystack.
* `grep` - Measures a count of all matching lines in a haystack.
* `grep-captures` - Measures a count of all matching capturing groups for
every line in a haystack.
* `regex-redux` - A port of the
[Benchmark Game's `regex-redux` program][regex-redux].

Note that these are the models supported by the implementations of each regex
engine found in this repository. If other tooling wants to reuse this same
format, it is not required that their benchmark models match the ones listed
here.

More details on each of the benchmark models supported by `rebar` can be found
in the [MODELS][models] document.

[regex-redux]: https://benchmarksgame-team.pages.debian.net/benchmarksgame/description/regexredux.html
[models]: MODELS.md

### `name`

The local name of the benchmark within a group. It must match the regex
`^[-A-Za-z0-9]+$`.

The group name of a benchmark is constructed by collecting the parent
directories up to but not including `{bench_dir}/definitions`, along with the
basename of the TOML file containing the benchmark with the `.toml` suffix
stripped. This sequence is then joined together with `/`.

The full name of a benchmark is `{group}/{name}`. The full name of a benchmark
must be unique with respect to all other benchmark definitions within the
benchmark directory.

### `regex`

The `regex` field defines zero or more regex patterns that will be measured.
The field can accept a single string, an array of strings or a table that
permits additional configuration. If it's a single string, then it corresponds
to defining a single regex with the default configuration. If it's an array,
then it corresponds to defining zero or more regexes with the default
configuration.

The "full" or table variant has the following fields:

* `patterns` - Either a single string or an array of strings that define the
patterns.
* `path` - A path to a file containing the regex (or regexes, depending on the
options). If `path` is present, then `patterns` must not be. When read from a
file, the contents are first trimmed of whitespace.
* `literal` - Whether to treat the regex pattern as a literal. Enabling this
will cause each pattern to have all special meta characters escaped before
giving it to the regex engine to compile.
* `per-line` - Specifies how to read regexes from a file. This only has an
effect when `path` is present. When `per-line` is absent, then the file is
interpreted as one single regex, with leading and trailing whitespace trimmed.
When `per-line` is present, then the file is treated as a sequence of lines and
its value corresponds to how to interpret each line. When set to `alternate`,
then the lines are joined together using `|` as a delimiter to form a single
pattern. When set to `pattern`, then each line is treated as a single pattern.
* `prepend` - Prepend the string to the beginning of each pattern.
* `append` - Append the string to the end of each pattern.

Not all regex engines support searching for multiple regular expressions. If
you try to include such a regex engine in a benchmark with multiple regular
expressions, then capturing a measurement will fail.

Note that specifying _zero_ patterns is valid. A regex containing zero patterns
never matches anything. While this is obviously pathological, it may still be
interesting to measure. There also isn't any compelling reason to ban it.

The `regex-redux` model embeds its own regex patterns into the model itself,
and so providing a non-empty value for this benchmark will result in an
error.

Here are some examples. This first one defines multiple patterns with the
default configuration:

```toml
regex = ["[a-zA-Z]{6}", "[0-9]{6}"]
```

This one reads a single pattern from a file at `{bench_dir}/regexes/foo/quux`:

```toml
regex = { path = "foo/quux" }
```

This one reads a dictionary of literal words into a single pattern, with
the words separated by a `|`:

```toml
regex = { path = "dictionary", literal = true, per-line = "alternate" }
```

### `case-insensitive`

When enabled, the regex is treated case insensitively. If the regex engine
doesn't support this option, then a measurement error will occur for that
engine. One should prefer this option in lieu of a `(?i)` flag. While most
regex engines support inline flags, not all of them do. (Such as `regress`.)

When absent, this defaults to `false`.

### `unicode`

When enabled, the regex is built with "Unicode mode" enabled. If the regex
engine doesn't support this option, then implementors should decide whether
the regex engine supports Unicode mode by default. For example, Go's standard
library regexp engine doesn't have a Unicode mode that one can enabled, but it
does support some basic Unicode features by default. So Go's regexp engine will
work regardless of whether `unicode` is enabled or not.

Unicode mode refers to things like the following:

* `.` never matches invalid UTF-8 and instead matches entire codepoints.
* Character classes like `\pL` are available.
* For some regex engines, `\w` is Unicode aware and much bigger than its
traditional ASCII-only definition. (Note that some regex engines, like RE2,
won't ever make `\w` Unicode aware even when Unicode mode is enabled. For this
reason, many of the benchmarks across multiple engines that enable Unicode
mode will avoid using `\w`.)
* Case insensitive searching takes, at least, Unicode "simple case folding"
rules into account.
* Character classes like `[^a]` match any codepoint except for `a`, rather than
any byte except for `a`.

When absent, this defaults to `false`.

### `haystack`

The `haystack` field defines what the regex should search. Other than the
`compile` benchmark, searching the haystack represents the fundamental unit of
work that is being measured.

The field can either be a string, or it can be a table that permits additional
configuration. The table has the following fields:

* `contents` - A string corresponding to the haystack.
* `path` - A path to a file containing the haystack. If `path` is present,
then `contents` must not be. When read from a file, the haystack corresponds
precisely to the contents of the file, including any leading or trailing
whitespace. Using `path` is the only way to define a benchmark that contains
invalid UTF-8 since TOML strings must be valid UTF-8.
* `utf8-lossy` - When enabled, the haystack is lossily converted to UTF-8.
Any invalid UTF-8 sequences are replaced with `U+FFFD`, the Unicode replacement
codepoint, by the substitution of maximal subparts strategy.
* `trim` - Leading and trailing whitespace is trimmed from the haystack when
enabled.
* `line-start` - Ignore all lines before `line-start`, where the first line
starts at `0`. This is applied after `trim`, but before `repeat`.
* `line-end` - Ignore all lines at and after `line-end`. This is applied after
`trim`, but before `repeat`.
* `repeat` - Repeat the haystack contents this many times. This is applied
after `trim` but before `prepend` and `append`.
* `prepend` - The given string is automatically prepended to the haystack. This
occurs after trimming and repetition, if enabled.
* `append` - The given string is automatically appended to the haystack. This
occurs after trimming and repetition, if enabled.

Here are some examples. This first one defines a simple haystack using a TOML
string:

```toml
haystack = "foobar"
```

This is precisely equivalent, but uses the full table format:

```toml
haystack = { contents = "foobar" }
```

This defines a haystack that is located in a file at
`{bench_dir}/haystacks/foo/bar.txt`, is stripped of leading and trailing
whitespace and has the string `Sherlock Holmes` appended to the end of it:

```toml
haystack = { path = "foo/bar.txt", trim = true, append = "Sherlock Holmes" }
```

The `trim`, `prepend` and `append` options are particularly useful for reusing
the same haystack file for different benchmarks using small tweaks.

### `count`

A required field that specifies a count for verifying the results of the
benchmark. Its meaning differs slightly depending on the model:

* `compile` - The `count` refers to the number of non-overlapping matches in
the haystack. Note that the time it takes to produce the matches is not part of
the measurement for this model. The count is only used to verify that the regex
produces the expected results.
* `count` - For the plain `count` model, the `count` field refers to the total
number of non-overlapping matches in the haystack.
* `count-spans` - The `count` fields refers to the sum of the lengths (in
bytes) of all non-overlapping matches in a haystack.
* `count-captures` - The `count` field refers to the total number of
non-overlapping matching capturing groups. For example, running the regex
`([0-9])([0-9])|([a-z])` against `12a34` should produce a count of `8`. (The
count includes the implicit capturing group corresponding to the overall match.
Therefore, the number of matching capturing groups is always at least the total
number of matches.)
* `grep` - Like the `count` benchmark, but refers to the total number of
matching lines. This only counts each line once, even if the regex matches
multiple times within a line.
* `grep-captures` - Like the `count-captures` benchmark, but executes the
search per line. Unlike the `grep` model, this includes all matches within
each line.
* `regex-redux` - While this model embeds its own verification, benchmarks
should report the total length (in bytes) of the input after all replacements
have been made.

The value for this field is usually just an integer, and this is what should
generally be used whenever possible. In some cases though, regex engines will
have slightly different match counts. One can express different counts for
different engines by using an array of tables as a value, where each table
has the following keys:

* `engine` - A regex that matches against the engine name. The regex is
automatically wrapped in `^` and `$` anchors. If an engine doesn't match one of
the regex patterns given, then an error is raised.
* `count` - The integer count for the specific engine.

The `engine` regex patterns are matched in order. That is, the first pattern
to match is the count that will be used.

For example, this specifies a count of `27` for the `hyperscan` engine, and
`5` for all others:

```toml
count = [
    { engine = "hyperscan", count = 27 },
    { engine = ".*", count = 5 },
]
```

Authors of benchmarks with varying counts across different regex engines should
be careful to check that they are benchmarking apples-to-apples. Or if they're
not, a comment should explain what's going on and why if possible. Namely,
counts are a critical part of benchmarking as they provide _some_ assurance
that regex engines are doing roughly equivalent work. It is very easy to
misconfigure a regex engine or misunderstand a subtle semantic that leads to
different match counts that might in turn lead to measuring something other
than what is intended.

### `engines`

This corresponds to an array of regex engines for which to collect measurements
for this benchmark. Each regex engine is specified by a string and must match
the regex `^[-A-Za-z0-9]+(/[-A-Za-z0-9]+)*$`. The `engines` field is
required and must be non-empty.

Every entry in this array must correspond to an engine defined in
`{bench_dir}/engines.toml`.
