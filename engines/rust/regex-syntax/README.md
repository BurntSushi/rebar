This directory contains a Rust runner program for benchmarking operations from
the [`regex-syntax` crate][rust-regex-syntax]. Since syntax operations do not
actually produce regex engines themselves, this runner program only supports
the `compile` benchmark model.

Currently, this runner program supports two operations:

* `ast` - Measures the time it takes to parse the concrete syntax of a regex
(that is, an `&str`) into an abstract syntax tree (AST).
* `hir` - Measures the time it takes to translate an abstract sytnax tree into
a high-level intermediate representation (HIR).

Unlike many regex engines, the `regex-syntax` crate has a "true" AST in
that it can faithfully roundtrip regex patterns, and this helps support the
construction of good error messages. Many regex engines try to combine these
two things into one step. The cost for splitting them apart, I think, is
that it tends to overall be slower to do this. But compilation into an `NFA`
(measured via the `rust/regex/nfa` engine) generally dwarfs this phase anyway.

[rust-regex-syntax]: https://github.com/rust-lang/regex/tree/master/regex-syntax
