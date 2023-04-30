This directory contains a Rust runner program for benchmarking
[`regex-automata`][rust-regex-automata]. The `regex-automata` crate provides an
"expert" level API to the [Rust regex crate internals][rust-regex]. Indeed, the
`regex` crate is mostly just a thin wrapper around the `meta::Regex` API from
the `regex-automata` crate.

Therefore, most of what is said in [the `rust/regex` engine
README](../regex/README.md) applies here.

One thing worth mentioning is that this runner program does not just benchmark
the `meta::Regex` API from `regex-automata`. There wouldn't be too much of a
point to that, since as I mentioned, the `regex` crate is just a thin wrapper
around it. Indeed, this runner program actually benchmarks several different
regex engines from the `regex-automata` crate:

* `backtrack` - The bounded backtracker regex engine, which gets some of the
speed of a simple backtracking approach but trade `O(m*n)` heap space in
favor of avoiding exponential worst case time complexity. This is used in
`meta::Regex` for resolving capture groups when the combination of regex and
haystack is small enough.
* `dense` - A fully compiled DFA using a "dense" representation for its
transition table. This is only used in `meta::Regex` for very small regexes,
since DFAs tend to use a lot of memory and have worst case exponential time
complexity to build.
* `hybrid` - A hybrid NFA/DFA or "lazy DFA." This is the primary work horse
of `meta::Regex`. It builds its transition table at search time, but bounds
the size of the table. It sidesteps the construction time problem of full
DFAs by guaranteeing that at most one new transition is added per byte of
haystack searched. There are also heuristics in place to detect when the cache
of transitions is being used inefficiently, and will fall back to a different
regex engine.
* `meta` - A meta regex engine that combines many other regex engines together
(along with prefilters) in order to execute searches as quickly as possible.
Generally this shouldn't be used as this is what the `rust/regex` runner
program uses.
* `nfa` - This is not actually a regex engine and only supports the `compile`
benchmark model. This simply measures how long it takes to build a Thompson NFA
from a regex's high-level intermediate representation (HIR).
* `onepass` - A fully compiled DFA with at most one state for every NFA state.
It can only be built from one-pass NFAs, which have the property that at any
step during a search, there is always at most one NFA state to transition to
next. This only supports anchored regexes and is typically used as the fastest
way inside a `meta::Regex` to report capture group positions for regexes that
are one-pass.
* `pikevm` - The "basic" NFA simulation with Rob Pike's extension to support
reporting capture group positions. This is generally the slowest engine to
execute in practice, but it supports everything.
* `sparse` - Like `dense`, but uses a sparse representation for the transition
table. This can save substantial space, but makes searching a bit slower.
Currently, `meta::Regex` does not use this.

[rust-regex-automata]: https://docs.rs/regex-automata
[rust-regex]: https://github.com/rust-lang/regex
