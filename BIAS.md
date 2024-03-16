This documents what I believe to be the bias of this barometer. Bias is
important to explicity describe because bias influences how one might interpret
results. For example, if the author of the barometer is also the author of
one of the regex engines included in the baromer (as is the case here), then
it's reasonable to assume that bias may implicitly or explicitly influence the
results to favor that regex engine.

The following is a list of biases that I was able to think of. Contributions
expanding this list are welcome.

* As mentioned above, I ([@BurntSushi]) authored both this barometer and the
[Rust regex crate]. The fact that the regex crate does well in this barometer
should perhaps be treated suspiciously. For example, even assuming good faith,
I may have selected a set of benchmarks that I knew well, and have thus spent
time optimizing for.
* The barometer represents a _curation_ of benchmarks, which implies someone
had to make a decision about not only which benchmarks to include, but also
which to exclude. Even if I hadn't also authored a regex engine included in
this barometer, this selection process would still be biased. My hope is that
this can be mitigated over time as the curated benchmarks are refined. We
should not add to the curated set without bound, but I do expect modifications
to it to be made. Ideally, the curated set would somehow approximate the set
of all regular expressions being executed in the wild, but this is of course
difficult to ascertain. So we wind up having to make guesses, and thus, bias
is introduced. I've also attempted to mitigate this bias by orienting some
proportion of benchmarks on regexes I've found used in other projects. (And of
course, the selection of those benchmarks is surely biased as well.)
* The analysis presented for each benchmark is heavily geared towards the
`rust/regex` engine. This is because I know that engine the best. I've also
found it somewhat difficult to understand what other engines actually do. The
source code of most regex engines is actually quite difficult to casually
browse. I often find it most difficult to get a high level picture of what's
happening. With that said, profiling programs can usually lead one to make
educated guesses as to what an engine is doing. Nevertheless, I welcome
contributions for improving analysis.
* The specific set of [benchmark models](MODELS.md) used represents bias
in how things are actually measured. While we try to do better than other
regex benchmarks by including multiple different types of measurements, we
of course cannot account for everything. For example, one common technique
used in practice, especially with automata oriented regex engines, is to run
one simpler regex that might produce false positives and then another more
complex regex to eliminate the false positives that get by the first. This
might be because of performance, or simply because of a lack of features (like
look-around). Another example of a model that is not included is one that both
compiles and searches for a regex as a single unit of work. We instead split
this apart into separate "compile" and "search" models.
* The author of this barometer has a background principally in automata
oriented regex engines. For this reason, all benchmarks in this barometer
measure true _regular_ expressions. More than that, they essentially avoid
any fancy features that are not known how to implement efficiently, such as
arbitrary look-around. (Simple look-around assertions like `^`, `$` and `\b`
are used though.) This means that the barometer misses a whole classes of
regexes that are just not measured here at all.
* The benchmarking setup works by repeatedly executing the same task over and
over again, with nothing changing. This can result in an artificial measurement
because things are usually changing in the real world. For example, in the
real world, it's pretty unlikely that one is running the same regex against
the same haystack repeatedly. Instead, it's likely that at least the haystack
is changing in some way. _Some_ benchmark models account for this, namely the
`grep` models, by running the regex on each line. The `count` model, however,
just repeats the same regex search on the same haystack over and over again.
Similarly, the `compile` model builds a regex from the same pattern over and
over again. This methodology is probably fine in most cases, but it does seem
to result in flawed measurements where an exceptionally well tuned JIT or
caching mechanism is in place that cannot be easily cleared.

[@BurntSushi]: https://github.com/BurntSushi
[Rust regex crate]: https://github.com/rust-lang/regex
