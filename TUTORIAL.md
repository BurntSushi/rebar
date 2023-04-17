This document provides a guided exploration through several of rebar's
sub-commands.

## Table of contents

* [Run benchmarks and gather measurements](#run-benchmarks-and-gather-measurements)
* [Explore measurements](#explore-measurements)
* [Rank regex engines](#rank-regex-engines)

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

Assuming you have Cargo, Go and Python installed, you should see some output
like the following:

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
benchmark name. If we use both `-e/--engine` and `-f/--filter` like we do
above, then the benchmark has to match both of them in order to run.

Notice here that I used `tee` to both write the measurement results to `stdout`
_and_ capture them to a file. I did that because measurement results can be
used as an input to other rebar sub-commands for inspection. Since measurement
results can take a long time to record, it's nice to both be able to see the
progress and to capture them for inspection later.

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

All we need to do exploratory analysis on measurements is `rebar` and a
checkout of this repository. The [BUILD](BUILD.md) document explains that
in more detail.

Assuming your current working directory is at the root of the repository, we
can start working with a saved set of recorded measurements:

```
$ ls -l record/all/2023-04-11/*.csv | wc -l
30
```

Typically, the way to look at a set of measurements is to compare the results
for each benchmark across each regex engine. The command to do that is `rebar
cmp`. You just need to give it all of the measurements:

```
$ rebar cmp record/all/2023-04-11/*.csv
```

If you run that command, you'll likely quickly see the problem: the table is
just too big. There are too many benchmarks and too many regex engines. There
are a few ways to deal with this.

Since the rows for the table are the benchmarks (i.e., each column reflects the
results for a single regex engine), you can make the table more readable by
limiting the regex engines one examines with the `-e/--engine` flag. For
example, to compare the `rust/regex`, `re2` and `go/regexp` engines across all
benchmarks:

```
$ rebar cmp record/all/2023-04-11/*.csv -e '^(re2|rust/regex|go/regexp)$'
```

The above command shows all benchmarks where _at least one_ of the engines has
a recorded result. You might instead only want to look at results where all
three engines have a result:

```
$ rebar cmp record/all/2023-04-11/*.csv -e '^(re2|rust/regex|go/regexp)$' --intersection
```

The `--intersection` flag is a filter that says, "only show results where every
regex engine has a measurement."

But sometimes looking at all results, even for a smaller set of engines, can be
overwhelming. What about only looking at benchmarks for which there is a big
difference between engines? The `-t/--threshold-min` and `-T/--threshold-max`
flags let's you filter results down to "benchmarks that contain at least one
results whose speedup ratio compared to the best is within the threshold range
given." The threshold flags are usually useful when doing a comparison between
two engines, but it works for any number. For example, we might only want to
see benchmarks in which there is at least one result that is 50 times slower
than the best:

```
$ rebar cmp record/all/2023-04-11/*.csv -e '^(re2|rust/regex)$' -t 50
```

Or perhaps we might only want to see benchmarks in which there is at least
one result that is within 1.2 times the speed of the best:

```
$ rebar cmp record/all/2023-04-11/*.csv -e '^(re2|rust/regex)$' -T 1.20
```

But what if we want to flip all of this around and look at all of the regex
engines, but only for a small set of benchmarks? In this case, the `--row`
flag can be used to flip the rows and colummns. That is, the rows become regex
engines and the columns become benchmarks. We use the `-f/--filter` flag to
limit our benchmarks (otherwise we'd still have the problem of too many columns
because there are so many benchmarks):

```
$ rebar cmp record/all/2023-04-11/*.csv --row engine -f mariomka -f regex-redux
engine              imported/mariomka/email  imported/mariomka/uri  imported/mariomka/ip  imported/regex-redux/regex-redux
------              -----------------------  ---------------------  --------------------  --------------------------------
dotnet              -                        -                      -                     223.01ms (17.91x)
dotnet/compiled     30.4 GB/s (1.74x)        8.2 GB/s (1.00x)       1777.3 MB/s (8.61x)   62.28ms (5.00x)
dotnet/nobacktrack  26.9 GB/s (1.96x)        3.5 GB/s (2.37x)       705.1 MB/s (21.71x)   65.55ms (5.27x)
go/regexp           46.6 MB/s (1160.30x)     46.9 MB/s (179.06x)    30.4 MB/s (503.58x)   407.51ms (32.73x)
hyperscan           37.6 GB/s (1.40x)        4.1 GB/s (2.01x)       14.9 GB/s (1.00x)     77.04ms (6.19x)
icu                 30.8 MB/s (1757.19x)     34.3 MB/s (245.30x)    574.2 MB/s (26.66x)   -
java/hotspot        62.1 MB/s (870.02x)      73.0 MB/s (115.08x)    53.0 MB/s (288.82x)   137.21ms (11.02x)
javascript/v8       163.8 MB/s (329.80x)     213.5 MB/s (39.35x)    10.4 GB/s (1.44x)     25.35ms (2.04x)
pcre2               87.8 MB/s (615.69x)      94.4 MB/s (88.98x)     819.4 MB/s (18.68x)   158.03ms (12.69x)
pcre2/jit           497.9 MB/s (108.52x)     496.8 MB/s (16.91x)    2.5 GB/s (5.98x)      21.00ms (1.69x)
perl                159.9 MB/s (337.83x)     154.9 MB/s (54.24x)    581.3 MB/s (26.33x)   151.96ms (12.21x)
python/re           46.3 MB/s (1167.51x)     77.3 MB/s (108.69x)    40.5 MB/s (377.63x)   135.95ms (10.92x)
python/regex        20.6 MB/s (2622.48x)     36.2 MB/s (231.98x)    736.2 MB/s (20.79x)   202.29ms (16.25x)
re2                 986.8 MB/s (54.76x)      891.1 MB/s (9.43x)     982.3 MB/s (15.58x)   30.96ms (2.49x)
regress             88.5 MB/s (610.89x)      99.0 MB/s (84.90x)     105.3 MB/s (145.43x)  63.68ms (5.11x)
rust/regex          825.6 MB/s (65.45x)      765.6 MB/s (10.97x)    721.5 MB/s (21.22x)   13.09ms (1.05x)
rust/regex/meta     52.8 GB/s (1.00x)        7.9 GB/s (1.04x)       3.0 GB/s (5.05x)      12.45ms (1.00x)
rust/regexold       824.6 MB/s (65.53x)      765.6 MB/s (10.97x)    3.0 GB/s (4.95x)      14.88ms (1.20x)
```

Here we use `-f/--filter` twice to say "show any benchmarks containing either
`mariomka` or `regex-redux` in the name."

We can also change the aggregate statistic used for comparison with the
`-s/--statistic` flag. By default, the `median` is used. But we could use the
minimum instead. The minimum applies to the minimum absolute timing, and since
we show throughput by default, this in turn means that we show the maximum
recorded throughput.

```
$ rebar cmp record/all/2023-04-11/*.csv --row engine -f mariomka -f regex-redux -s min
```

Speaking of throughput, we can change the default such that only absolute
times are shown using the `-u/--units` flag:

```
$ rebar cmp record/all/2023-04-11/*.csv --row engine -f mariomka -f regex-redux -s min -u time
```

## Rank regex engines

If you want to try to get a sense of how regex engines do over a large corpus
of benchmarks, you can summarize the results using the `rebar rank` command. It
works by computing the speedup ratios of each regex engine for each benchmark,
relative to the fastest engine for each benchmark. The fastest engine has a
speedup ratio of `1.0`, and every other engine has a speedup ratio of `N`,
where the engine is said to be "`N` times slower than the fastest."

The `rebar rank` command then averages the speedup ratios for every engine
using the geometric mean. In other words, this distills the relative
performance of a single regex engine across many benchmarks down into a single
number.

Naively, you can ask for a ranking of regex engines across all recorded
results, and `rebar` will happily give it to you:

```
$ rebar rank record/all/2023-04-11/*.csv
```

The problem with this is that some regex engines are used in a lot more
benchmarks than others. Notably, `rust/regex` is used in a lot because the
author of this barometer also uses this tool for optimizing `rust/regex`, and
so it naturally has many benchmarks defined for it. Similarly, some engines
have very few benchmarks defined for it. More to the point, some engines, like
`rust/regex/hir`, are just measuring an aspect of compilation time for the
`rust/regex` engine and aren't relevant for search benchmarks.

In other words, asking for a ranking across all benchmarks in this barometer is
a category error. You _could_ make it a little better by limiting the ranking
to considering search-only benchmarks by excluding all benchmarks that use the
`compile` model:

```
$ rebar rank record/all/2023-04-11/*.csv -M compile
```

But this doesn't address all of the issues.

Another way to go about this is to limit yourself to a subset of benchmarks for
which every regex engine has healthy representation compared to the others. The
`curated` benchmarks are specifically designed for this (although are not
perfect at it, so the comparison is still flawed in some sense):

```
$ rebar rank record/all/2023-04-11/*.csv -M compile -f '^curated/'
```

You can make the comparison a little bit more rigorous by limiting it to the
set of benchmarks for which every regex engine has a result:

```
$ rebar rank record/all/2023-04-11/*.csv -M compile -f '^curated/' --intersection
```

It is also possible to do a correctish ranking across _all_ measurements if you
limit yourself to the benchmarks in common between each engine. In practice,
this is most useful in the context of a pairwise comparison, because it ensures
you get the most amount of data. (As you add more engines, the set of
benchmarks containing all of them starts to shrink.) We do still want to
exclude `compile` benchmarks. (You basically never want to run `rebar rank`
without either `-m compile` to include only compile-time benchmarks, or `-M
compile` to exclude only compile-time benchmarks. Namely, lumping compile and
search time benchmarks into the same ranking doesn't make a ton of sense.)

```
$ rebar rank record/all/2023-04-11/*.csv -M compile -e '^(hyperscan|rust/regex)$' --intersection
Engine      Version           Geometric mean of speed ratios  Benchmark count
------      -------           ------------------------------  ---------------
hyperscan   5.4.1 2023-02-22  1.33                            45
rust/regex  1.7.2             2.73                            45
```

And if you ever want to drill down into what precise benchmarks are
contributing to the above ranking, you can always flip to using `rebar cmp`. It
accepts the same types of filtering flags, so literally all you have to do is
use `rebar cmp` instead of `rebar rank`:

```
$ rebar cmp record/all/2023-04-11/*.csv -M compile -e '^(hyperscan|rust/regex)$' --intersection
```

Please be careful what conclusions you draw from `rebar rank`. It exists purely
because there is so much data, and it can be useful to get a "general sense" of
where regex engines stand with respect to one another. But remember, it is very
difficult to use it as a tool for definitively declaring one regex engine as
faster than another, _especially_ if the geometric means are anywhere near
close to one another. More to the point, [biased](BIAS.md) benchmark selection
directly influences what the geometric mean is able to tell you. Its signal is
only as good as the inputs given to it.
