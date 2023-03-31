rebar
=====
A biased barometer for gauging the relative speed of some regex engines on a
curated set of tasks.

## Results

This section shows the results of a _curated and [biased](BIAS.md)_ set of
benchmarks. These reflect only a small subset of the benchmarks defined in
this repository, but were carefully crafted to attempt to represent a broad
range of use cases and annotated where possible with analysis to aide in the
interpretation of results.

The results begin with a summary, then a list of links to each benchmark group
and then finally the results for each group. Results are shown one benchmark
group at a time, where a single group is meant to combine related regexes or
workloads, where it is intended to be useful to see how results change across
regex engines. Analysis is provided, at minimum, for every group. Although,
analysis is heavily biased towards Rust's regex crate, as it is what this
author knows best. However, contributions that discuss other regex engines are
very welcomed.

Below each group of results are the parameters for each individual benchmark
within that group. An individual benchmark may contain some analysis specific
to it, but it will at least contain a summary of the benchmark details. Some
parameters, such as the haystack, are usually too big to show in this README.
One can use rebar to look at the haystack directly. Just take the `full name`
of the benchmark and give it to the `rebar haystack` command. For example:

```
$ rebar haystack unicode/compile/fifty-letters
ͱͳͷΐάέήίΰαβγδεζηθικλμνξοπρςστυφχψωϊϋόύώϙϛϝϟϡϸϻͱͳͷΐάέή
```

Similarly, the full benchmark execution details (including the haystack) can
be seen with the `rebar klv` command:

```
$ rebar klv unicode/compile/fifty-letters
name:29:unicode/compile/fifty-letters
model:7:compile
pattern:7:\pL{50}
case-insensitive:5:false
unicode:4:true
haystack:106:ͱͳͷΐάέήίΰαβγδεζηθικλμνξοπρςστυφχψωϊϋόύώϙϛϝϟϡϸϻͱͳͷΐάέή
max-iters:1:0
max-warmup-iters:1:0
max-time:1:0
max-warmup-time:1:0
```

Finally, you can run the benchmark yourself and look at results on the command
line:

```
$ rebar measure -f '^unicode/compile/fifty-letters$' | tee results.csv
$ rebar cmp results.csv
```

<!-- BEGIN: report -->
<!-- Auto-generated by rebar, do not edit manually! -->
<!-- Generated with command: -->
<!-- rebar report tmp/base/2023-03-29.1/dotnet-compiled.csv tmp/base/2023-03-29.1/dotnet.csv tmp/base/2023-03-29.1/dotnet-nobacktrack.csv tmp/base/2023-03-29.1/go-regexp.csv tmp/base/2023-03-29.1/hyperscan.csv tmp/base/2023-03-29.1/java-hotspot.csv tmp/base/2023-03-29.1/javascript-v8.csv tmp/base/2023-03-29.1/pcre2.csv tmp/base/2023-03-29.1/pcre2-jit.csv tmp/base/2023-03-29.1/perl.csv tmp/base/2023-03-29.1/python-re.csv tmp/base/2023-03-29.1/python-regex.csv tmp/base/2023-03-29.1/re2.csv tmp/base/2023-03-29.1/regress.csv tmp/base/2023-03-29.1/rust-aho-corasick-dfa.csv tmp/base/2023-03-29.1/rust-aho-corasick-nfa.csv tmp/base/2023-03-29.1/rust-memchr-memmem.csv tmp/base/2023-03-29.1/rust-regex-ast.csv tmp/base/2023-03-29.1/rust-regex-backtrack.csv tmp/base/2023-03-29.1/rust-regex.csv tmp/base/2023-03-29.1/rust-regex-dense.csv tmp/base/2023-03-29.1/rust-regex-hir.csv tmp/base/2023-03-29.1/rust-regex-hybrid.csv tmp/base/2023-03-29.1/rust-regex-meta.csv tmp/base/2023-03-29.1/rust-regex-nfa.csv tmp/base/2023-03-29.1/rust-regexold.csv tmp/base/2023-03-29.1/rust-regex-onepass.csv tmp/base/2023-03-29.1/rust-regex-pikevm.csv tmp/base/2023-03-29.1/rust-regex-sparse.csv tmp/icu.csv --splice README.md --statistic median --units throughput -f ^curated/ --intersection -->
### Summary

Below are two tables summarizing the results of regex engines benchmarked.
Each regex engine includes its version at the time measurements were captured,
a summary score that ranks it relative to other regex engines across all
benchmarks and the total number of measurements collected.

The first table ranks regex engines based on search time. The second table
ranks regex engines based on compile time.

The summary statistic used is the [geometric mean] of the speed ratios for
each regex engine across all benchmarks that include it. The ratios within
each benchmark are computed from the median of all timing samples taken, and
dividing it by the best median of the regex engines that participated in the
benchmark. For example, given two regex engines `A` and `B` with results `35
ns` and `25 ns` on a single benchmark, `A` has a speed ratio of `1.4` and
`B` has a speed ratio of `1.0`. The geometric mean reported here is then the
"average" speed ratio for that regex engine across all benchmarks.

Each regex engine is linked to the directory containing the runner program
responsible for compiling a regex, using it in a search and reporting timing
results. Each directory contains a `README` file briefly describing any engine
specific details for the runner program.

Each regex engine is also defined in
[benchmarks/engines.toml](benchmarks/engines.toml), using the same name listed
in the table below. Each definition includes instructions for how to run,
build, clean and obtain the version of each regex engine.

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

#### Summary of search-time benchmarks

| Engine | Version | Geometric mean of speed ratios | Benchmark count |
| ------ | ------- | ------------------------------ | --------------- |
| [rust/regex/meta](engines/rust/regex-automata) | 0.2.0 | 1.47 | 15 |
| [rust/regex](engines/rust/regex) | 1.7.1 | 2.59 | 15 |
| [rust/regexold](engines/rust/regex-old) | 1.7.1 | 2.66 | 15 |
| [hyperscan](engines/hyperscan) | 5.4.1 2023-02-22 | 2.84 | 15 |
| [dotnet/compiled](engines/dotnet) | 7.0.3 | 3.50 | 15 |
| [pcre2/jit](engines/pcre2) | 10.42 2022-12-11 | 8.81 | 15 |
| [dotnet/nobacktrack](engines/dotnet) | 7.0.3 | 8.88 | 15 |
| [re2](engines/re2) | 2023-03-01 | 9.68 | 15 |
| [javascript/v8](engines/javascript) | 19.7.0 | 17.09 | 15 |
| [regress](engines/regress) | 0.5.0 | 45.45 | 15 |
| [python/re](engines/python) | 3.10.9 | 49.15 | 15 |
| [python/regex](engines/python) | 2022.10.31 | 52.61 | 15 |
| [perl](engines/perl) | 5.36.0 | 68.73 | 15 |
| [icu](engines/icu) | 72.1.0 | 83.26 | 15 |
| [java/hotspot](engines/java) | 20+36-2344 | 102.17 | 15 |
| [go/regexp](engines/go) | 1.20.1 | 105.88 | 15 |
| [pcre2](engines/pcre2) | 10.42 2022-12-11 | 443.15 | 15 |

#### Summary of compile-time benchmarks

| Engine | Version | Geometric mean of speed ratios | Benchmark count |
| ------ | ------- | ------------------------------ | --------------- |

### Benchmark Groups

Below is a list of links to each benchmark group in this particular barometer.
Each benchmark group contains 1 or more related benchmarks. The idea of each
group is to tell some kind of story about related workloads, and to give
a sense of how performance changes based on the variations between each
benchmark.

* [literal](#literal)
* [literal-alternate](#literal-alternate)
* [cloud-flare-redos](#cloud-flare-redos)
* [aws-keys](#aws-keys)
* [bounded-repeat](#bounded-repeat)

### literal

This group of benchmarks measures regex patterns that are simple literals. When
possible, we also measure case insensitive versions of the same pattern. We do
this across three languages: English, Russian and Chinese. For English, Unicode
mode is disabled while it is enabled for Russian and Chinese. (Which mostly
only matters for the case insensitive benchmarks.)

This group is mainly meant to demonstrate two things. Firstly is whether the
regex engine does some of the most basic forms of optimization by recognizing
that a pattern is just a literal, and that a full blown regex engine is
probably not needed. Indeed, naively using a regex engine for this case is
likely to produce measurements much worse than most regex engines. Secondly is
how the performance of simple literal searches changes with respect to both
case insensitivity and Unicode. Namely, substring search algorithms that work
well on ASCII text don't necessarily also work well on UTF-8 that contains many
non-ASCII codepoints. This is especially true for case insensitive searches.

Notice, for example, how RE2 seems to be faster in the `sherlock-casei-ru`
benchmark than in the `sherlock-ru` benchmark, even though the latter is "just"
a simple substring search where as the former is a multiple substring search.
In the case of `sherlock-ru`, RE2 actually attempts a literal optimization that
likely gets caught up in dealing with a high false positive rate of candidates.
Where as in the case of `sherlock-casei-ru`, no literal optimization is
attempted and instead its lazy DFA is used. The high false positive rate in the
simpler literal case winds up making it overall slower than it likely would be
if it would just use the DFA.

This is not in any way to pick on RE2. Every regex engine that does literal
optimizations (and most do) will suffer from this kind of setback in one way
or another.

| Engine | sherlock-en | sherlock-casei-en | sherlock-ru | sherlock-casei-ru | sherlock-zh |
| - | - | - | - | - | - |
| dotnet/compiled | 12.4 GB/s | 6.1 GB/s | 20.5 GB/s | 5.1 GB/s | 32.2 GB/s |
| dotnet/nobacktrack | 7.8 GB/s | 4.0 GB/s | 7.1 GB/s | 2.4 GB/s | 28.8 GB/s |
| go/regexp | 3.9 GB/s | 43.2 MB/s | 2.1 GB/s | 32.8 MB/s | 2033.6 MB/s |
| hyperscan | **34.5 GB/s** | **30.4 GB/s** | 4.4 GB/s | 7.3 GB/s | **50.7 GB/s** |
| icu | 1596.3 MB/s | 453.7 MB/s | 3.0 GB/s | 281.5 MB/s | 4.2 GB/s |
| java/hotspot | 2.4 GB/s | 280.3 MB/s | 3.9 GB/s | 222.9 MB/s | 5.2 GB/s |
| javascript/v8 | 5.2 GB/s | 2.9 GB/s | **41.0 GB/s** | 3.3 GB/s | 11.0 GB/s |
| pcre2 | 7.1 GB/s | 968.2 MB/s | 2.1 MB/s | 2039.6 KB/s | 57.6 MB/s |
| pcre2/jit | 26.3 GB/s | 16.9 GB/s | 31.9 GB/s | **18.9 GB/s** | 35.7 GB/s |
| perl | 2.8 GB/s | 556.9 MB/s | 3.3 GB/s | 98.3 MB/s | 7.7 GB/s |
| python/re | 3.7 GB/s | 295.7 MB/s | 6.7 GB/s | 455.3 MB/s | 11.2 GB/s |
| python/regex | 3.6 GB/s | 3.0 GB/s | 4.7 GB/s | 4.1 GB/s | 6.8 GB/s |
| re2 | 10.6 GB/s | 2.5 GB/s | 764.2 MB/s | 942.0 MB/s | 2.7 GB/s |
| regress | 3.5 GB/s | 1196.3 MB/s | 3.6 GB/s | 314.0 MB/s | 3.6 GB/s |
| rust/regex | 31.1 GB/s | 9.4 GB/s | 33.0 GB/s | 6.0 GB/s | 40.0 GB/s |
| rust/regex/meta | 28.6 GB/s | 9.3 GB/s | 31.1 GB/s | 9.0 GB/s | 40.1 GB/s |
| rust/regexold | 31.9 GB/s | 8.0 GB/s | 30.8 GB/s | 6.8 GB/s | 38.7 GB/s |

<details>
<summary>Show individual benchmark parameters.</summary>

**sherlock-en**

| Parameter | Value |
| --------- | ----- |
| full name | `curated/01-literal/sherlock-en` |
| model | [`count`](MODELS.md#count) |
| regex | `````Sherlock Holmes````` |
| case-insensitive | `false` |
| unicode | `false` |
| haystack-path | [`opensubtitles/en-sampled.txt`](benchmarks/haystacks/opensubtitles/en-sampled.txt) |
| count(`.*`) | 513 |


**sherlock-casei-en**

| Parameter | Value |
| --------- | ----- |
| full name | `curated/01-literal/sherlock-casei-en` |
| model | [`count`](MODELS.md#count) |
| regex | `````Sherlock Holmes````` |
| case-insensitive | `true` |
| unicode | `false` |
| haystack-path | [`opensubtitles/en-sampled.txt`](benchmarks/haystacks/opensubtitles/en-sampled.txt) |
| count(`.*`) | 522 |


**sherlock-ru**

| Parameter | Value |
| --------- | ----- |
| full name | `curated/01-literal/sherlock-ru` |
| model | [`count`](MODELS.md#count) |
| regex | `````Шерлок Холмс````` |
| case-insensitive | `false` |
| unicode | `true` |
| haystack-path | [`opensubtitles/ru-sampled.txt`](benchmarks/haystacks/opensubtitles/ru-sampled.txt) |
| count(`.*`) | 724 |


**sherlock-casei-ru**

| Parameter | Value |
| --------- | ----- |
| full name | `curated/01-literal/sherlock-casei-ru` |
| model | [`count`](MODELS.md#count) |
| regex | `````Шерлок Холмс````` |
| case-insensitive | `true` |
| unicode | `true` |
| haystack-path | [`opensubtitles/ru-sampled.txt`](benchmarks/haystacks/opensubtitles/ru-sampled.txt) |
| count(`.*`) | 746 |


**sherlock-zh**

| Parameter | Value |
| --------- | ----- |
| full name | `curated/01-literal/sherlock-zh` |
| model | [`count`](MODELS.md#count) |
| regex | `````夏洛克·福尔摩斯````` |
| case-insensitive | `false` |
| unicode | `true` |
| haystack-path | [`opensubtitles/zh-sampled.txt`](benchmarks/haystacks/opensubtitles/zh-sampled.txt) |
| count(`.*`) | 30 |


</details>

### literal-alternate

This group is like `literal`, but expands the complexity from a simple literal
to a small alternation of simple literals, including case insensitive variants
where applicable. Once again, we do this across three languages: English,
Russian and Chinese. We disable Unicode mode for English but enable it for
Russian and Chinese. Enabling Unicode here generally only means that case
insensitivity takes Unicode case folding rules into account.

This benchmark ups the ante when it comes to literal optimizations. Namely,
for a regex engine to optimize this case, it generally needs to be capable of
reasoning about literal optimizations that require one or more literals from
a set to match. Many regex engines don't deal with this case well, or at all.
For example, after a quick scan at comparing the `sherlock-en` benchmark here
and in the previous `literal` group, one thing that should stand out is the
proportion of regex engines that now measure throughput in MB/s instead of
GB/s.

One of the difficulties in optimizing for this case is that multiple substring
search is difficult to do in a way that is fast. In particular, this benchmark
carefully selected each alternation literal to start with a different character
than the other alternation literals. This, for example, inhibits clever regex
engines from noticing that all literals begin with the same byte (or small
number of bytes). Consider an alternation like `foo|far|fight`. It is not hard
to see that a regex engine _could_ just scan for the letter `f` as a prefilter
optimization. Here, we pick our regex such that this sort of shortcut isn't
available. For the regex engine to optimize this case, it really needs to deal
with the problem of multiple substring search.

Multiple substring search _can_ be implemented via a DFA, and perhaps in some
cases, quite quickly via a [shift DFA]. Beyond that though, multiple substring
search can be implemented by other various algorithms such as Aho-Corasick or
Rabin-Karp. (The standard Aho-Corasick formulation is an NFA, but it can also
be converted to a DFA by pre-computing all failure transitions. This winds up
with a similar result as using Thompson's construction to produce an NFA and
then powerset construction to get a DFA, but the Aho-Corasick construction
algorithm is usually quite a bit faster because it doesn't need to deal with a
full NFA.)

The problem here is that DFA speeds may or may not help you. For example, in
the case of RE2 and Rust's regex engine, it will already get DFA speeds by
virtue of their lazy DFAs. Indeed, in this group, RE2 performs roughly the same
across all benchmarks. So even if you, say build an Aho-Corasick DFA, it's not
going to help much if at all. So it makes sense to avoid it.

But Rust's regex crate has quite a bit higher throughputs than RE2 on most of
the benchmarks in this group. So how is it done? Currently, this is done via
the [Teddy] algorithm, which was ported out of [Hyperscan]. It is an algorithm
that makes use of SIMD to accelerate searching for a somewhat small set of
literals. Most regex engines don't have this sort of optimization, and indeed,
it seems like Teddy is not particularly well known. Alas, regex engines that
want to move past typical DFA speeds for multiple substring search likely need
some kind of vectorized algorithm to do so. (Teddy is also used by Rust's
regex crate in the previous `literal` group of benchmarks for accelerating
case insensitive searches. Namely, it enumerates some finite set of prefixes
like `she`, `SHE`, `ShE` and so on, and then looks for matches of those as a
prefilter.)

[shift DFA]: https://gist.github.com/pervognsen/218ea17743e1442e59bb60d29b1aa725
[Teddy]: https://github.com/BurntSushi/aho-corasick/tree/4e7fa3b85dd3a3ce882896f1d4ee22b1f271f0b4/src/packed/teddy
[Hyperscan]: https://github.com/intel/hyperscan

| Engine | sherlock-en | sherlock-casei-en | sherlock-ru | sherlock-casei-ru | sherlock-zh |
| - | - | - | - | - | - |
| dotnet/compiled | 3.7 GB/s | 456.2 MB/s | 2.2 GB/s | 784.2 MB/s | 16.1 GB/s |
| dotnet/nobacktrack | 2.6 GB/s | 374.5 MB/s | 1005.2 MB/s | 293.7 MB/s | 10.4 GB/s |
| go/regexp | 24.7 MB/s | 15.5 MB/s | 32.2 MB/s | 9.0 MB/s | 45.7 MB/s |
| hyperscan | **16.6 GB/s** | **15.2 GB/s** | 4.6 GB/s | **3.5 GB/s** | **20.0 GB/s** |
| icu | 649.7 MB/s | 112.8 MB/s | 168.5 MB/s | 107.1 MB/s | 334.4 MB/s |
| java/hotspot | 69.0 MB/s | 64.9 MB/s | 109.1 MB/s | 55.2 MB/s | 184.3 MB/s |
| javascript/v8 | 686.1 MB/s | 675.3 MB/s | 936.1 MB/s | 587.4 MB/s | 6.4 GB/s |
| pcre2 | 866.2 MB/s | 160.3 MB/s | 1742.0 KB/s | 1629.7 KB/s | 8.6 MB/s |
| pcre2/jit | 1564.4 MB/s | 654.6 MB/s | 1161.1 MB/s | 296.6 MB/s | 2.5 GB/s |
| perl | 1086.6 MB/s | 114.5 MB/s | 113.1 MB/s | 80.1 MB/s | 225.5 MB/s |
| python/re | 439.8 MB/s | 36.5 MB/s | 411.5 MB/s | 54.5 MB/s | 1031.1 MB/s |
| python/regex | 298.8 MB/s | 72.6 MB/s | 310.7 MB/s | 78.6 MB/s | 873.7 MB/s |
| re2 | 923.8 MB/s | 922.6 MB/s | 936.1 MB/s | 930.3 MB/s | 965.0 MB/s |
| regress | 1609.8 MB/s | 310.7 MB/s | 275.3 MB/s | 116.2 MB/s | 284.2 MB/s |
| rust/regex | 15.1 GB/s | 2.7 GB/s | 2.9 GB/s | 516.5 MB/s | 18.6 GB/s |
| rust/regex/meta | 10.8 GB/s | 3.0 GB/s | **6.6 GB/s** | 1670.6 MB/s | 15.1 GB/s |
| rust/regexold | 16.5 GB/s | 2.8 GB/s | 3.0 GB/s | 453.9 MB/s | 15.3 GB/s |

<details>
<summary>Show individual benchmark parameters.</summary>

**sherlock-en**

| Parameter | Value |
| --------- | ----- |
| full name | `curated/02-literal-alternate/sherlock-en` |
| model | [`count`](MODELS.md#count) |
| regex | `````Sherlock Holmes\|John Watson\|Irene Adler\|Inspector Lestrade\|Professor Moriarty````` |
| case-insensitive | `false` |
| unicode | `false` |
| haystack-path | [`opensubtitles/en-sampled.txt`](benchmarks/haystacks/opensubtitles/en-sampled.txt) |
| count(`.*`) | 714 |


**sherlock-casei-en**

| Parameter | Value |
| --------- | ----- |
| full name | `curated/02-literal-alternate/sherlock-casei-en` |
| model | [`count`](MODELS.md#count) |
| regex | `````Sherlock Holmes\|John Watson\|Irene Adler\|Inspector Lestrade\|Professor Moriarty````` |
| case-insensitive | `true` |
| unicode | `false` |
| haystack-path | [`opensubtitles/en-sampled.txt`](benchmarks/haystacks/opensubtitles/en-sampled.txt) |
| count(`.*`) | 725 |


**sherlock-ru**

| Parameter | Value |
| --------- | ----- |
| full name | `curated/02-literal-alternate/sherlock-ru` |
| model | [`count`](MODELS.md#count) |
| regex | `````Шерлок Холмс\|Джон Уотсон\|Ирен Адлер\|инспектор Лестрейд\|профессор Мориарти````` |
| case-insensitive | `false` |
| unicode | `true` |
| haystack-path | [`opensubtitles/ru-sampled.txt`](benchmarks/haystacks/opensubtitles/ru-sampled.txt) |
| count(`.*`) | 899 |


**sherlock-casei-ru**

| Parameter | Value |
| --------- | ----- |
| full name | `curated/02-literal-alternate/sherlock-casei-ru` |
| model | [`count`](MODELS.md#count) |
| regex | `````Шерлок Холмс\|Джон Уотсон\|Ирен Адлер\|инспектор Лестрейд\|профессор Мориарти````` |
| case-insensitive | `true` |
| unicode | `true` |
| haystack-path | [`opensubtitles/ru-sampled.txt`](benchmarks/haystacks/opensubtitles/ru-sampled.txt) |
| count(`.*`) | 971 |


**sherlock-zh**

| Parameter | Value |
| --------- | ----- |
| full name | `curated/02-literal-alternate/sherlock-zh` |
| model | [`count`](MODELS.md#count) |
| regex | `````夏洛克·福尔摩斯\|约翰华生\|阿德勒\|雷斯垂德\|莫里亚蒂教授````` |
| case-insensitive | `false` |
| unicode | `true` |
| haystack-path | [`opensubtitles/zh-sampled.txt`](benchmarks/haystacks/opensubtitles/zh-sampled.txt) |
| count(`.*`) | 207 |


</details>

### cloud-flare-redos

This benchmark uses a regex that helped cause an [outage at
Cloudflare][cloudflare-blog]. This class of vulnerability is typically called a
"regular expression denial of service," or "ReDoS" for short. It doesn't always
require a malicious actor to trigger. Since it can be difficult to reason about
the worst case performance of a regex when using an unbounded backtracking
implementation, it might happen entirely accidentally on valid inputs.

The particular regex that contributed to the outage was:

```
(?:(?:"|'|\]|\}|\\|\d|(?:nan|infinity|true|false|null|undefined|symbol|math)|`|\-|\+)+[)]*;?((?:\s|-|~|!|\{\}|\|\||\+)*.*(?:.*=.*)))
```

As discussed in Cloudflare's post mortem, the specific problematic portion of
the regex is:

```
.*(?:.*=.*)
```

Or more simply:

```
.*.*=.*;
```

We benchmark the original regex along with the simplified variant. We also
split the simplified variant into one with a short haystack (about 100 bytes)
and one with a long haystack (about 10,000 bytes). The benchmark results for
the original and simplified short variant should be roughly similar, but the
difference between the short and long variant is where things get interesting.
The automata based engines generally maintain a similar throughput for both the
short and long benchmarks, but the backtrackers slow way down. This is because
the backtracking algorithm for this specific regex and haystack doesn't scale
linearly with increases in the size of the haystack.

The purpose of this benchmark is to show a real world scenario where the use of
a backtracking engine can bite you in production if you aren't careful.

We include Hyperscan in this benchmark, although it is questionable to do so.
Hyperscan reports many overlapping matches from the regex used by Cloudflare
because of the trailing `.*`, so it is probably not a great comparison.
In particular, this regex was originally used in a firewall, so it seems
likely that it would be used in a "is a match" or "not a match" scenario. But
our benchmark here reproduces the analysis in the appendix of Cloudflare's
port mortem. But the real utility in including Hyperscan here is that it
demonstrates that it is not a backtracking engine. While its throughput is not
as high as some other engines, it remains roughly invariant with respect to
haystack length, just like other automata oriented engines.

Note that `rust/regex` has very high throughput here because the regex is
small enough to get compiled into a full DFA. The compilation process also
"accelerates" some states, particularly the final `.*`. This acceleration works
by noticing that almost all of the state's transitions loop back on itself, and
only a small number transition to another state. The final `.*` for example
only leaves its state if it sees the end of the haystack or a `\n`. So the DFA
will actually run `memchr` on `\n` and skip right to the end of the haystack.

[cloudflare-blog]: https://blog.cloudflare.com/details-of-the-cloudflare-outage-on-july-2-2019/

| Engine | original | simplified-short | simplified-long |
| - | - | - | - |
| dotnet/compiled | 130.7 MB/s | 876.3 MB/s | 13.2 GB/s |
| dotnet/nobacktrack | 12.8 MB/s | 189.3 MB/s | 289.4 MB/s |
| go/regexp | 41.8 MB/s | 44.4 MB/s | 49.1 MB/s |
| hyperscan | 85.8 MB/s | 82.4 MB/s | 85.0 MB/s |
| icu | 3.4 MB/s | 3.5 MB/s | 42.7 KB/s |
| java/hotspot | 5.8 MB/s | 6.3 MB/s | 76.6 KB/s |
| javascript/v8 | 19.4 MB/s | 18.9 MB/s | 335.3 KB/s |
| pcre2 | 2.9 MB/s | 2.8 MB/s | 30.2 KB/s |
| pcre2/jit | 50.0 MB/s | 42.5 MB/s | 671.2 KB/s |
| perl | 10.4 MB/s | 10.1 MB/s | 176.7 KB/s |
| python/re | 22.3 MB/s | 21.9 MB/s | 337.6 KB/s |
| python/regex | 6.2 MB/s | 6.0 MB/s | 91.8 KB/s |
| re2 | 347.1 MB/s | 333.1 MB/s | 493.7 MB/s |
| regress | 9.2 MB/s | 8.9 MB/s | 115.3 KB/s |
| rust/regex | 455.5 MB/s | 498.8 MB/s | 599.9 MB/s |
| rust/regex/meta | **570.1 MB/s** | **1768.6 MB/s** | **81.0 GB/s** |
| rust/regexold | 468.1 MB/s | 493.8 MB/s | 588.7 MB/s |

<details>
<summary>Show individual benchmark parameters.</summary>

**original**

| Parameter | Value |
| --------- | ----- |
| full name | `curated/06-cloud-flare-redos/original` |
| model | [`count-spans`](MODELS.md#count-spans) |
| regex | `````(?:(?:"\|'\|\]\|\}\|\\\|\d\|(?:nan\|infinity\|true\|false\|null\|undefined\|symbol\|math)\|`\|-\|\+)+[)]*;?((?:\s\|-\|~\|!\|\{\}\|\\|\\|\|\+)*.*(?:.*=.*)))````` |
| case-insensitive | `false` |
| unicode | `false` |
| haystack | `math x=xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx [.. snip ..]` |
| count(`hyperscan`) | 5757 |
| count(`.*`) | 107 |


**simplified-short**

| Parameter | Value |
| --------- | ----- |
| full name | `curated/06-cloud-flare-redos/simplified-short` |
| model | [`count-spans`](MODELS.md#count-spans) |
| regex | `````.*.*=.*````` |
| case-insensitive | `false` |
| unicode | `false` |
| haystack | `x=xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx [.. snip ..]` |
| count(`hyperscan`) | 5252 |
| count(`.*`) | 102 |


**simplified-long**

| Parameter | Value |
| --------- | ----- |
| full name | `curated/06-cloud-flare-redos/simplified-long` |
| model | [`count-spans`](MODELS.md#count-spans) |
| regex | `````.*.*=.*````` |
| case-insensitive | `false` |
| unicode | `false` |
| haystack-path | [`cloud-flare-redos.txt`](benchmarks/haystacks/cloud-flare-redos.txt) |
| count(`hyperscan`) | 50004999 |
| count(`.*`) | 10000 |


</details>

### aws-keys

This [measures a regex][pypi-aws-secrets-regex] for [detecting AWS keys in
source code][pypi-aws-secrets-regex][aws-key-blog]. In particular, to reduce
false positives, it looks for both an access key and a secret key within a few
lines of one another.

We also measure a "quick" version of the regex that is used to find possible
candidates by searching for things that look like an AWS access key.

The measurements here demonstrate why the [pypi-aws-secrets] project splits
this task into two pieces. First it uses the "quick" version to identify
candidates, and then it uses the "full" version to lower the false positive
rate of the "quick" version. The "quick" version of the regex runs around
an order of magnitude faster than the "full" version across the board. To
understand why, let's look at the "quick" regex:

```
((?:ASIA|AKIA|AROA|AIDA)([A-Z0-7]{16}))
```

Given this regex, every match starts with one of `ASIA`, `AKIA`, `AROA` or
`AIDA`. This makes it quite amenable to prefilter optimizations where a regex
engine can look for matches of one of those 4 literals, and only then use the
regex engine to confirm whether there is a match at that position. Some regex
engines will also notice that every match starts with an `A` and use `memchr`
to look for occurrences of `A` as a fast prefilter.

We also include compilation times to give an idea of how long it takes
to compile a moderately complex regex, and how that might vary with the
compilation time of a much simpler version of the regex.

Note that in all of the measurements for this group, we search the CPython
source code (concatenated into one file). We also lossily convert it to UTF-8
so that regex engines like `regress` can participate in this benchmark. (The
CPython source code contains a very small amount of invalid UTF-8.)

[pypi-aws-secrets]: https://github.com/pypi-data/pypi-aws-secrets
[pypi-aws-secrets-regex]: https://github.com/pypi-data/pypi-aws-secrets/blob/903a7bd35bc8d9963dbbb7ca35e8ecb02e31bed4/src/scanners/mod.rs#L15-L23
[aws-key-blog]: https://tomforb.es/i-scanned-every-package-on-pypi-and-found-57-live-aws-keys/

| Engine | quick |
| - | - |
| dotnet/compiled | 811.3 MB/s |
| dotnet/nobacktrack | 683.3 MB/s |
| go/regexp | 864.5 MB/s |
| hyperscan | 1372.7 MB/s |
| icu | 338.2 MB/s |
| java/hotspot | 117.9 MB/s |
| javascript/v8 | 293.0 MB/s |
| pcre2 | 1468.2 MB/s |
| pcre2/jit | 1029.2 MB/s |
| perl | 139.7 MB/s |
| python/re | 164.1 MB/s |
| python/regex | 117.2 MB/s |
| re2 | 997.4 MB/s |
| regress | 719.0 MB/s |
| rust/regex | 1468.9 MB/s |
| rust/regex/meta | **1814.4 MB/s** |
| rust/regexold | 1406.3 MB/s |

<details>
<summary>Show individual benchmark parameters.</summary>

**quick**

| Parameter | Value |
| --------- | ----- |
| full name | `curated/09-aws-keys/quick` |
| model | [`grep`](MODELS.md#grep) |
| regex | `````((?:ASIA\|AKIA\|AROA\|AIDA)([A-Z0-7]{16}))````` |
| case-insensitive | `false` |
| unicode | `false` |
| haystack-path | [`wild/cpython-226484e4.py`](benchmarks/haystacks/wild/cpython-226484e4.py) |
| count(`.*`) | 0 |


</details>

### bounded-repeat

This group of benchmarks measures how well regex engines do with bounded
repeats. Bounded repeats are sub-expressions that are permitted to match
up to some fixed number of times. For example, `a{3,5}` matches 3, 4 or 5
consecutive `a` characters. Unlike unbounded repetition operators, the regex
engine needs some way to track when the bound has reached its limit. For this
reason, many regex engines will translate `a{3,5}` to `aaaa?a?`. Given that
the bounds may be much higher than `5` and that the sub-expression may be much
more complicated than a single character, bounded repeats can quickly cause the
underlying matcher to balloon in size.

We measure three different types of bounded repeats:

* A search for a number of consecutive letters, both ASCII only and Unicode
aware.
* A search for certain types of words surrounding a `Result` type in Rust
source code.
* A search for consecutive words, all beginning with a capital letter.

We also include measurements for the compilation time of the last two.

Hyperscan does unusually well here, particularly for an automata oriented
engine. It's plausible that it has some specific optimizations in place for
bounded repeats.

`rust/regex` slows down quite a bit on the `context` regex. Namely, the
`context` regex is quite gnarly and its `(?s:.)` sub-expression coupled with
the bounded repeat causes a large portion of its transition table to get filled
out. This in turn results in more time than usual being spent actually building
the lazy DFA's transition table during a search. Typically, the lazy DFA's
transition table is built pretty quickly and then mostly reused on subsequent
searches. But in this case, the transition table exceeds the lazy DFA's cache
capacity and results in the cache getting cleared. However, the rate at which
new transitions are created is still low enough that the lazy DFA is used
instead of falling back to a slower engine.

| Engine | letters-en |
| - | - |
| dotnet/compiled | 258.3 MB/s |
| dotnet/nobacktrack | 146.0 MB/s |
| go/regexp | 29.4 MB/s |
| hyperscan | **736.0 MB/s** |
| icu | 51.1 MB/s |
| java/hotspot | 92.6 MB/s |
| javascript/v8 | 149.0 MB/s |
| pcre2 | 71.9 MB/s |
| pcre2/jit | 333.2 MB/s |
| perl | 67.8 MB/s |
| python/re | 73.0 MB/s |
| python/regex | 35.6 MB/s |
| re2 | 495.9 MB/s |
| regress | 168.1 MB/s |
| rust/regex | 623.4 MB/s |
| rust/regex/meta | 694.6 MB/s |
| rust/regexold | 612.0 MB/s |

<details>
<summary>Show individual benchmark parameters.</summary>

**letters-en**

| Parameter | Value |
| --------- | ----- |
| full name | `curated/10-bounded-repeat/letters-en` |
| model | [`count`](MODELS.md#count) |
| regex | `````[A-Za-z]{8,13}````` |
| case-insensitive | `false` |
| unicode | `false` |
| haystack-path | [`opensubtitles/en-sampled.txt`](benchmarks/haystacks/opensubtitles/en-sampled.txt) |
| count(`hyperscan`) | 3724 |
| count(`.*`) | 1833 |


</details>

<!-- END: report -->

## Wanted

It would be great to add more regex engines to this barometer. I am thinking
of at least the following, but I'm generally open to any regex engine that
has a reasonable build process with stable tooling:

* Ruby's regex engine, or perhaps just [Onigmo](https://github.com/k-takata/Onigmo)
directly.
* [`nim-regex`](https://github.com/nitely/nim-regex)
* [D's std.regex](https://dlang.org/phobos/std_regex.html)
* [CTRE](https://github.com/hanickadot/compile-time-regular-expressions). (This
one may prove tricky since "compile a regex" probably means "compile a C++
program." The rebar tool supports this, but it will be annoying. If you want
to add this, please file an issue to discuss an implementation path.)
* A POSIX regex engine.
* [`NSRegularExpression`](https://developer.apple.com/documentation/foundation/nsregularexpression), perhaps through Swift?
* Lisp's [CL-PPCRE](https://github.com/edicl/cl-ppcre/).
* A selected subset of the [mess that is regex libraries for
Haskell](https://wiki.haskell.org/Regular_expressions).

Here are some other regex engines I'm aware of, but I have reservations about
including them:

* PHP's `preg` functions. This "just" uses PCRE2, which is already included
in this benchmark, so it's not clear whether it's also worth measuring here
too. But maybe it is. Maybe PHP introduces some interesting performance
characteristics that meaningfully alter the picture presented by using PCRE2
directly.
* Julia's standard regex engine, which last I checked was also PCRE2. So a
similar reasoning as for PHP applies here.
* C++'s `std::regex` or Boost's regex engine. These are known to be horribly
slow. Maybe we should still include them for completeness.
* [re2c](http://re2c.org/) does regex matching through code generation, so this
would likely work similarly to CTRE if it were to be added. It serves a very
different use case than most regex engines, so I'm not sure if it fits here,
but it could be interesting.
* Regex engines embedded in grep tools like GNU grep. These may be a little
tricky to correctly benchmark given the methodology here, but I think it
should be possible to satisfy at least some of the models. The idea here would
be to actually call the `grep` program and not try to rip the regex engine
out of it.
* Tcl's regex library is something I've benchmarked in the past and I recall
it also being extraordinarily slow. So I'm not sure if it's worth it? Also,
does anyone still choosing Tcl for new projects in production?

I think the main criteria for inclusion are:

* Someone has to actually be using the regex engine for something. It's not
scalable to include every regex engine someone threw up on GitHub five years
ago that isn't being maintained and nobody is using.
* The build process for the regex engine needs to be somewhat reasonable,
or it needs to somehow be maintained by someone else. For example, we don't
build Python's regex engine or ICU. Instead, we just require that Python or
ICU be installed by some other means, with Python being made available via the
`python` binary, and ICU being made available via `pkg-config`.
