This directory contains runner programs for regex engines benchmarked by
rebar. Some runner programs provide measurements for multiple regex engines
(for example, [python/main.py](python/main.py) measures both the standard
library `re` module and the third party `regex` module) while other runner
programs provide measurements for just one regex engine.

Every runner program works by accepting a benchmark definition in the [KLV
format](../KLV.md) on `stdin`, gathering samples for the corresponding
[benchmark model](../MODELS.md) repeatedly, and printing each sample on
`stdout` as a pair of `duration` (in nanoseconds) and `count` (used to verify
the benchmark executed correctly) values. The `rebar` tool then uses these
runner programs to measure regex engine performance.

The idea here is that the runner programs should be as simple as possible, and
generally should not use any dependencies other than a programming language's
standard library (and the regex engine, of course, if it is not in the standard
library). This is why there's no JSON, or TOML or YAML or anything else that
usually needs a third party library if it isn't in std. There's no reading
or writing to files. No CLI arg parsing. Just a dead simple KLV formatted
benchmark definition on stdin and a comma delimited result output on stdout.

We also obtain simplicity by mostly avoiding error handling. While all runner
programs should have defined behavior for all inputs, errors do not need to be
human friendly. Namely, since the runner programs are specifically designed
only for use with rebar, it can be generally assumed that the input on stdin
and the way in which the program is run will be correct. That is, the runner
programs don't need to facilitate direct human iteraction (although it can be
quite useful to directly interact with runner programs, and the `rebar klv`
command enables this).

If the regex engine is written in C or C++ or can otherwise be exposed over a
C API with little effort, then it is generally recommended to write the runner
program in Rust for the following reasons:

1. It will be easier for @BurntSushi to maintain.
2. Rust has a zero cost C foreign function interface, so there should not be
any overhead.
3. The build process will be owned by Cargo's `build.rs` and should hopefully
avoid introducing things like autotools or cmake.
4. The [shared](../shared) directory has several helpers for writing runner
programs in Rust (because there are many Rust runner programs) that make Rust
runner programs a bit simpler than other languages.

Otherwise, if a Rust runner program doesn't work well (or at all), then it is
acceptable to use another language. Indeed, enabling the use of non-Rust runner
programs is a key motivation of rebar's sub-process architecture.

## Using a runner program directly

This example shows how to build and run the `rust/regex` engine runner program
directly for a specific benchmark. This also shows the output format.

```
$ rebar build -e '^rust/regex$'
$ rebar klv imported/sherlock/repeated-class-negation --max-time 3s --max-iters 10 \
  | ./engines/rust/regex/target/release/main
15033876,2130
1226248,2130
1253532,2130
1222738,2130
1222676,2130
1279095,2130
1222645,2130
1222246,2130
1233423,2130
1249894,2130
```

The first command builds just the `rust/regex` engine runner program. The
second command is composed of a `rebar klv` command and the `rust/regex`
runner program that was just built. The `rebar klv` command reads the
`imported/sherlock/repeated-class-negation` benchmark definition and converts
it to the KLV format. The `--max-time 3s` and `--max-iters 10` are also
included in the KLV data, and instruct the runner program how long to
execute the benchmark and how many times to execute it. (The runner program
stops when the first limit is reached. So in this case, the runner program
will try to never run the benchmark for more than 3 seconds and never more
than 10 iterations. Note though, that runner programs are not expected to
"interrupt" the regex engine. If a regex engine takes so long that a single
measurement exceeds a reasonable time limit, then either the engine isn't worth
benchmarking or the benchmark definition needs to be adjusted.)

The `rust/regex` runner program then carries out its instructions and outputs
two pieces of data for each sample it collects. The first is the number of
nanoseconds it took to execute a single iteration of the benchmark. The second
is the "count" returned by the benchmark in order to verify that its results
are what one expects. (The "count" is computed in different ways depending on
the [model](../MODELS.md) being used.)

The runner program does not need to stream samples to stdout. It may collect
them all in memory before printing them.

If a runner program cannot get the current time in nanoseconds, then whatever
environment you're in probably won't work with rebar since many of the
benchmarks defined execute in less than 1 microsecond.
