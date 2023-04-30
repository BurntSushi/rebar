This directory contains a Rust runner program for benchmarking [Rust's
regex-lite crate][rust-regex-lite]. The `regex-lite` crate is authored by the
same person who created this barometer.

This runner program makes the following decisions:

* Only one pattern is supported.
* Currently, only haystacks that are valid UTF-8 are supported. At the time of
writing, `regex-lite` only supports searching `&str`.

[rust-regex-lite]: https://github.com/rust-lang/regex/tree/master/regex-lite

## Unicode

The `regex-lite` crate specifically gives up functionality and performance in
exchange for faster compilation times and smaller binary size. To this end, the
extent to which `regex-lite` supports Unicode is that its fundamental atom of
matching is a single codepoint. That is, it knows about UTF-8. But it doesn't
have any other Unicode support. Case insensitive searching, `\w`, `\d`, `\s`
and `\b` are all ASCII-only. There is no support for Unicode character classes
such as `\pL`. Note though that `[^a]` matches any codepoint other than `a` by
necessity of being aware of UTF-8.
