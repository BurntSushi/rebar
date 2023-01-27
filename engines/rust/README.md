This directory contains runner programs for a number of crates related to
[Rust's regex crate][rust-regex]. Briefly:

* `aho-corasick` measures an implementation of the Aho-Corasick algorithm for
multiple substring search.
* `memchr` measures an implementation of a vectorized single substring search
algorithm.
* `regex` measures Rust's regex crate API.
* `regex-automata` measures a number of regex engines internal to Rust's regex
crate.
* `regex-old` measures a snapshot of Rust's regex crate before a large
internal refactoring.
* `regex-syntax` measures the time it takes to parse a regular expression.

[rust-regex]: https://github.com/rust-lang/regex
