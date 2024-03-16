This directory contains a Rust runner program for benchmarking a pinned and
older version of [Rust's regex crate][rust-regex]. Most of what is said in
[the `rust/regex` engine README](../regex/README.md) applies here.

The main motivation for this regex engine was to compare the performance of the
regex crate both before and after a [huge internal change]. It's possible that
as time goes by this comparison will become less relevant and this regex engine
will be removed from this barometer.

[rust-regex]: https://github.com/rust-lang/regex
[huge internal change]: https://github.com/rust-lang/regex/issues/656
