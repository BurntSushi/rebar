# Benchmark Execution

This document describes the format for benchmark execution. In contrast to
benchmark definitions, the format for benchmark execution describes how to
actually run and capture measurements for a benchmark, while a definition
describes the parameters of the benchmark.

A benchmark execution _includes_ a benchmark definition, but also includes
other parameters such as maximum iterations and maximum time that should be
spent on the benchmark execution.

The format used for this is a human readable variant of "key-length-value" or
"KLV." This format is used to convey the full benchmark execution information
to a program that executes the regex engine. Therefore, this format is only
relevant to authors of regex engine harness programs and isn't relevant to
users defining new benchmarks.

The idea of a KLV format is that it consists of a number of key-value pairs.
Each pair starts with a key, then a length follows and finally a value with a
number of bytes corresponding to the length.

## Design constraints

The constraints that led to the design of this format are as follows:

* There is almost no need for it to be fast to read or write. The amount of
data being transmitted is usually relatively small (at most a few megabytes),
and neither reading nor writing this format is part of any measurement
performed by rebar.
* A need to transmit a flattened structure of keys and values, while supporting
the ability to provide multiple regex patterns.
* A strong desire to make it simple to parse in almost any programming language
without any external dependencies. Hence why something like TOML or even JSON
was not used.
* While it's not necessary for it to be human writable, I wanted it to be
human readable so that it could be easily inspected. Usually KLV formats are
binary formats, but the human readable requirement lead to it being in a plain
text format.

## Format details

The actual format is a possibly empty sequence of key-length-value triples.
Each triple has this format:

* A UTF-8 encoded key name that does not contain a `:`.
* A `:`.
* A decimal formatted integer indicating the `length`, in bytes, of the value.
* A `:`.
* The value, which may be arbitrary bytes but must contain exactly `length`
bytes.
* A `\n`. (This serves no purpose other than to make the format easier for a
human to read.)

The following keys are used by rebar. None of the keys are required by the
format, although in practice leaving some of them absent (like `model`) should
ultimately result in the harness program reporting an error.

* `name` - The name of the benchmark.
* `model` - The benchmark model to use.
* `pattern` - A regex pattern. All regex patterns must be valid UTF-8. This
key may be given zero or more times. Most regex engines only support a single
pattern, and so harness programs exposing such engines should return an error
if `pattern` is specified less than or more than once.
* `case-insensitive` - A boolean indicating whether the regex should match
case insensitively or not. Valid values are `true` or `false`.
* `unicode` - A boolean indicating whether the regex should match in "Unicode
mode" or not. Valid values are `true` or `false`.
* `haystack` - The bytes for the regex to search. This can be arbitrary bytes.
There is no requirement for it to be valid UTF-8. Some regex engines may
require valid UTF-8 to execute, in which case, benchmark definitions that
specify non-UTF-8 haystacks shouldn't list that engine for measurement. If it
does, the harness program should return an error.
* `max-iters`: The maximum number of iterations to run the benchmark.
* `max-warmup-iters`: The maximum number of warmup iterations to run before
measuring benchmark time.
* `max-time`: The approximate maximum time that should be spent running the
benchmark.
* `max-warmup-time`: The approximate maximum time that should be spent warming
up the benchmark.

In terms of benchmark execution, the first limit to be reached (whether it be
iterations or time) should result in the benchmark stopping. So for example,
if `max-iters = 1000000` and `max-time = 3s`, then an especially slow benchmark
that takes 1 second per iteration would only run approximately 3 iterations.
