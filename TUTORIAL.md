This document provides a guided exploration through several of rebar's
sub-commands.

Note that in order to run benchmarks, you need to build all of the regex engine
runner programs that you want to collect measurements for. If you just want to
explore some measurements, then you can skip to that section without needing to
build any of the regex engines. That is, one can explore the measurements that
are saved and committed to this repository without having to run any of the
benchmark programs. You just need to build `rebar` itself.

## Table of contents

* [Run benchmarks and gather measurements](#gather-measurements)
* [Explore measurements](#explore-measurements)

## Run benchmarks and gather measurements

For this section, you'll need to build both `rebar` and at least the regex
engines that you want to gather measurements for. Note that you don't need to
build all of the regex engines, but if you want to follow along with this
tutorial, you'll need to build `rust/regex`, `go/regexp` and `python/re`.

The [BUILD](BUILD.md) document explains the build process in more detail,
but quickly, you can build `rebar` like so:

```
$ git clone https://github.com/BurntSushi/rebar
$ cd rebar
$ cargo install --path .
```

and then build the regex engines we're interested in like this:

```
$ rebar build -e '^(rust/regex|go/regexp|python/re)$'
```

The `-e` flag is short for `--engine`, which can be used multiple times and
accepts a regex pattern. For example, this command is equivalent to the one
above:

```
$ rebar build -e '^rust/regex$' -e '^go/regexp$' -e '^python/re$'
```

The `-e/--engine` flag is accepted by most of rebar's sub-commands.

You should see some output like the following:

```
rust/regex: running: cd "engines/rust/regex" && "cargo" "build" "--release"
rust/regex: build complete for version 1.7.2
go/regexp: running: cd "engines/go" && "go" "build"
go/regexp: build complete for version 1.20.1
python/re: running: cd "engines/python" && "virtualenv" "ve"
python/re: build complete for version 3.10.9
```

Before collecting measurements, it might be a good idea to run rebar's test
suite for these engines. This ensures the regex engines are working in a
minimally expected way:

```
$ rebar measure -e '^(rust/regex|go/regexp|python/re)$' -f '^test/' --test
test/func/leftmost-first,count-spans,go/regexp,1.20.1,OK
test/func/leftmost-first,count-spans,python/re,3.10.9,OK
test/func/leftmost-first,count-spans,rust/regex,1.7.2,OK
test/func/dollar-only-matches-end,count,go/regexp,1.20.1,OK
test/func/dollar-only-matches-end,count,python/re,3.10.9,OK
test/func/dollar-only-matches-end,count,rust/regex,1.7.2,OK
[... snip ...]
```

In reality, running tests is almost like collecting measurements. The only
difference is that we provide the `-t/--test` flag, which tells rebar to
collect a single measurement for each regex engine and verify that its result
is what is expected.

Let's collect our first measurements. We'll run all of the benchmarks in the
`curated/04-ruff` group. This might take a little while to run, because each
benchmark repeats each task repeatedly until a certain number of iterations or
a time limit has beeen hit.

```
$ rebar measure -e '^(rust/regex|go/regexp|python/re)$' -f '^curated/04-ruff' | tee ruff.csv
name,model,rebar_version,engine,engine_version,err,haystack_len,iters,total,median,mad,mean,stddev,min,max
curated/04-ruff-noqa/real,grep-captures,0.0.1 (rev cef9e52192),go/regexp,1.20.1,,32514634,4,5.72s,933.94ms,180.57us,933.98ms,1.10ms,932.48ms,935.57ms
curated/04-ruff-noqa/real,grep-captures,0.0.1 (rev cef9e52192),python/re,3.10.9,,32514634,3,5.52s,1.08s,0.00ns,1.08s,785.70us,1.08s,1.08s
curated/04-ruff-noqa/real,grep-captures,0.0.1 (rev cef9e52192),rust/regex,1.7.2,,32514634,71,4.61s,42.44ms,0.00ns,42.45ms,137.39us,42.19ms,42.77ms
curated/04-ruff-noqa/tweaked,grep-captures,0.0.1 (rev cef9e52192),go/regexp,1.20.1,,32514634,70,4.66s,43.20ms,17.61us,43.47ms,2.11ms,41.00ms,58.82ms
curated/04-ruff-noqa/tweaked,grep-captures,0.0.1 (rev cef9e52192),python/re,3.10.9,,32514634,11,4.96s,283.11ms,0.00ns,283.14ms,759.22us,281.96ms,284.25ms
curated/04-ruff-noqa/tweaked,grep-captures,0.0.1 (rev cef9e52192),rust/regex,1.7.2,,32514634,128,4.56s,23.52ms,225.00ns,23.53ms,59.95us,23.37ms,23.69ms
curated/04-ruff-noqa/compile-real,compile,0.0.1 (rev cef9e52192),go/regexp,1.20.1,,,354872,4.67s,7.33us,0.00ns,7.61us,9.61us,2.41us,465.85us
curated/04-ruff-noqa/compile-real,compile,0.0.1 (rev cef9e52192),python/re,3.10.9,,,39818,4.57s,73.68us,0.00ns,73.77us,1.13us,71.05us,205.05us
curated/04-ruff-noqa/compile-real,compile,0.0.1 (rev cef9e52192),rust/regex,1.7.2,,,85849,4.57s,30.94us,0.00ns,30.98us,613.00ns,28.77us,39.90us
```

The `-f/--filter` flag accepts a regex pattern just like the `-e/--engine`
flag, but instead of being applied to the engine name, it's applied to the
benchmark name. If we use both `-e/--engine` and `-f/--filter` like we do here,
then the benchmark has to match both of them in order to run.

Notice here that I used `tee` to both write the measurement results to `stdout`
_and_ capture them to a file. I did that because measurement results can be
used as an input to other rebar sub-commands for inspection. Since measurement
results can take a long time to record, it's very useful to be able to do it
once and then inspect them many times.

The next section will discuss how to inspect the results. Namely, the raw CSV
data isn't particularly amenable for each analysis. You *can* look at it
directly if you chop it down a little bit and format it nicely with a tool
like [xsv](https://github.com/BurntSushi/xsv):

```
$ xsv select name,model,engine,iters,median,min ruff.csv | xsv table
name                               model          engine      iters   median    min
curated/04-ruff-noqa/real          grep-captures  go/regexp   4       933.94ms  932.48ms
curated/04-ruff-noqa/real          grep-captures  python/re   3       1.08s     1.08s
curated/04-ruff-noqa/real          grep-captures  rust/regex  71      42.44ms   42.19ms
curated/04-ruff-noqa/tweaked       grep-captures  go/regexp   70      43.20ms   41.00ms
curated/04-ruff-noqa/tweaked       grep-captures  python/re   11      283.11ms  281.96ms
curated/04-ruff-noqa/tweaked       grep-captures  rust/regex  128     23.52ms   23.37ms
curated/04-ruff-noqa/compile-real  compile        go/regexp   354872  7.33us    2.41us
curated/04-ruff-noqa/compile-real  compile        python/re   39818   73.68us   71.05us
curated/04-ruff-noqa/compile-real  compile        rust/regex  85849   30.94us   28.77us
```

Otherwise, there is one other filtering option worth mentioning. The
`-m/--model` flag lets you filter on the benchmark model used. This is often
useful, for example, to exclude measurements for compile time in the case where
you're only interested in search time benchmarks:

```
$ rebar measure -e '^(rust/regex|go/regexp|python/re)$' -f '^curated/04-ruff' -M compile | tee ruff-nocompile.csv
name,model,rebar_version,engine,engine_version,err,haystack_len,iters,total,median,mad,mean,stddev,min,max
curated/04-ruff-noqa/real,grep-captures,0.0.1 (rev cef9e52192),go/regexp,1.20.1,,32514634,4,5.67s,933.62ms,1.81ms,933.93ms,3.48ms,929.68ms,938.80ms
curated/04-ruff-noqa/real,grep-captures,0.0.1 (rev cef9e52192),python/re,3.10.9,,32514634,3,5.52s,1.08s,0.00ns,1.08s,1.13ms,1.08s,1.08s
curated/04-ruff-noqa/real,grep-captures,0.0.1 (rev cef9e52192),rust/regex,1.7.2,,32514634,71,4.61s,42.43ms,0.00ns,42.46ms,163.10us,42.22ms,43.06ms
curated/04-ruff-noqa/tweaked,grep-captures,0.0.1 (rev cef9e52192),go/regexp,1.20.1,,32514634,70,4.61s,42.89ms,13.13us,42.95ms,1.76ms,40.46ms,56.15ms
curated/04-ruff-noqa/tweaked,grep-captures,0.0.1 (rev cef9e52192),python/re,3.10.9,,32514634,11,5.01s,287.84ms,0.00ns,287.26ms,1.67ms,284.65ms,290.43ms
curated/04-ruff-noqa/tweaked,grep-captures,0.0.1 (rev cef9e52192),rust/regex,1.7.2,,32514634,130,4.56s,23.18ms,1.05us,23.19ms,59.18us,23.07ms,23.36ms
```

In this case, we actually used the inverse flag, `-M/--model-not`, to *exclude*
benchmarks with the string `compile` in their model name.

## Explore measurements

In this section, we explore measurements that have already been recorded. You
can of course use the very same techniques to explore your own measurements,
but by using pre-recorded measurements from this repository we make it possible
to do exploratory analysis without needing to build and run the benchmarks.
(For example, on my machine, collecting measurements for every benchmark takes
about 3 hours. Of course, you can always gather a subset of measurements too,
as explained in the previous section.)

All we need to do exploratory analysis on measurements is `rebar` itself. The
[BUILD](BUILD.md) document explains things in more detail, but as long as you
have Cargo installed, all you should need is this:

```
$ git clone https://github.com/BurntSushi/rebar
$ cd rebar
$ cargo install --path .
```
