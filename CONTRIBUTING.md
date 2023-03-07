This document describes the process for contributing to this barometer. We
mostly discuss the process for adding a new benchmark or a new regex engine,
but other changes are welcome as well. For small things, just submitting a pull
request is welcome. For bigger change requests or bug reports, please file an
issue. If you just have a question, then I encourage you to open a new
Discussion.

## Table of contents

* [Adding a new benchmark](#adding-a-new-benchmark)
* [Adding a new regex engine](#adding-a-new-regex-engine)

## Adding a new benchmark

**Summary:** Open a pull request with your new benchmark. If you're adding a
new curated benchmark that will appear in the README, be prepared to defend
your submission as it needs to clear a high bar. Your PR comment should include
the results of running your benchmark, probably by showing the output of a
`rebar cmp` command.

Before getting into whether a benchmark _should_ be added, here is a brief
description of a benchmark's nuts and bolts. Namely, all benchmarks in rebar
have the following things:

* A name.
* A regex.
* A haystack.
* A list of engines with which to run the regex.
* An expected result. (Usually a count of matches.)

All benchmarks are defined in TOML files in the
[`benchmarks/definitions`](./benchmarks/definitions) directory. In
cases where the regex is too big to fit in a TOML file, they are in the
[`benchmarks/regexes`](./benchmarks/regexes) directory. Similarly, in cases
where the haystack is too big to fit in a TOML (which is most of the time),
they are in the [`benchmarks/haystacks`](./benchmarks/haystacks) directory.

For more information about how the `benchmarks` directory is structured,
including a complete description of the supported TOML format, please see
the [`FORMAT.md`](FORMAT.md) document. With that said, it is likely simple
enough that you can look at existing definitions, copy one of them and then
work from there.

The question for whether a benchmark *should* be added or not is a tricky one.
Here are two guiding principles:

* Adding a new benchmark to
[`benchmarks/definitions/curated`](./benchmarks/definitions/curated) needs
to clear a high bar. These are the benchmarks that make up the public face
of the barometer, and are intended to give a broad overview of the
performance of various regex engines.
* Adding a new benchmark elsewhere only needs to clear a much lower bar. At
this point in time, my bar for it is "it's interesting in some way." Ideally
it wouldn't be duplicative of another benchmark, but in practice there are so
many benchmarks and determining whether any two actually overlap is actually
quite difficult.

Either way, the process for submitting a new benchmark should be to first open
a PR. The PR should include the full benchmark definition so that others can
checkout your branch and run the benchmark. Ideally, a benchmark will reuse
an existing haystack, but new ones can be added. We just need to be careful
not to add too many, as haystacks tend to be large and we don't want to bloat
the repository.

The next step is to run your new benchmark and present the results in the PR
for discussion. We use rebar for both. Let's say you added a new TOML file at
`benchmarks/definitions/wild/my-use-case.toml`. And in it, you defined this
benchmark:

```toml
[[bench]]
model = 'grep-captures'
name = 'shebang'
regex = '^(?P<spaces>\s*)#!(?P<directive>.*)'
unicode = true
haystack = { path = 'wild/cpython-226484e4.py' }
count = 282
engines = [
  'rust/regex',
  're2',
  'go/regexp',
  'pcre2',
  'python/re',
]
```

Once you've created that file, you're now ready to test that the results
reported by each engine match what you expect:

```
$ rebar measure -f wild/my-use-case/shebang --verify --verbose
wild/my-use-case/shebang,rust/regex,grep-captures,1.7.1,OK
wild/my-use-case/shebang,re2,grep-captures,2023-03-01,OK
wild/my-use-case/shebang,pcre2/jit,grep-captures,10.42 2022-12-11,OK
Traceback (most recent call last):
  File "/home/andrew/code/rust/rebar/engines/python/main.py", line 427, in <module>
    results = model_grep_captures(config)
  File "/home/andrew/code/rust/rebar/engines/python/main.py", line 244, in model_grep_captures
    h = c.get_haystack()
  File "/home/andrew/code/rust/rebar/engines/python/main.py", line 85, in get_haystack
    return self.haystack.decode('utf-8')
UnicodeDecodeError: 'utf-8' codec can't decode byte 0xf6 in position 10247181: invalid start byte
wild/my-use-case/shebang,grep-captures,python/re,3.10.8,failed to run command for 'python/re'
some benchmarks failed
```

Ah! One of the benchmarks failed. This is because `python/re` only supports
searching valid UTF-8 when `unicode = true`. You can either set `unicode =
true`, or force the haystack to be converted to UTF-8 lossily before taking
measurements:

```toml
haystack = { path = 'wild/cpython-226484e4.py' }
```

Now verification should succeed:

```
$ rebar measure -f wild/my-use-case/shebang --verify --verbose
wild/my-use-case/shebang,rust/regex,grep-captures,1.7.1,OK
wild/my-use-case/shebang,re2,grep-captures,2023-03-01,OK
wild/my-use-case/shebang,pcre2/jit,grep-captures,10.42 2022-12-11,OK
wild/my-use-case/shebang,python/re,grep-captures,3.10.8,OK
```

You're now ready to collect results. Results are reported on stdout. I usually
capture them in a file with `tee` so that I can see the results as they're
printed as well:

```
$ rebar measure -f wild/my-use-case/shebang | tee results.csv
name,model,engine,version,err,haystack_len,iters,total,median,mad,mean,stddev,min,max
wild/my-use-case/shebang,grep-captures,rust/regex,1.7.1,,32514634,26,4.56s,116.30ms,12.15us,116.19ms,592.97us,115.13ms,117.55ms
wild/my-use-case/shebang,grep-captures,re2,2023-03-01,,32514634,84,4.62s,36.04ms,1.07us,36.08ms,117.93us,35.89ms,36.43ms
wild/my-use-case/shebang,grep-captures,pcre2/jit,10.42 2022-12-11,,32514634,156,4.56s,19.26ms,469.00ns,19.27ms,78.63us,19.08ms,19.52ms
wild/my-use-case/shebang,grep-captures,python/re,3.10.8,,32514634,9,5.17s,358.15ms,0.00ns,358.35ms,712.38us,357.21ms,360.02ms
```

Ah, but unless you look really carefully, these results are not particularly
easy to interpret. Instead, we can ask rebar to do a comparison for us:

```
$ rebar cmp results.csv
benchmark                 pcre2/jit            python/re           re2                 rust/regex
---------                 ---------            ---------           ---                 ----------
wild/my-use-case/shebang  1610.0 MB/s (1.00x)  86.6 MB/s (18.60x)  860.4 MB/s (1.87x)  266.6 MB/s (6.04x)
```

For your PR, include `results.csv` and the output of `rebar cmp` above in your
comment. That should make the benchmark and its results easier to review.

## Adding a new regex engine

TODO. See the [`engines`](./engines/) directory for existing examples.
