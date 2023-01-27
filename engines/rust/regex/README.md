This directory contains a Rust runner program for benchmarking [Rust's regex
crate][rust-regex]. The regex crate is authored by the same person who created
this barometer, and the barometer's early motivation was for tracking and
improving the performance of the regex crate.

This runner program makes the following decisions:

* Only one pattern is supported. The `RegexSet` API is not benchmarked here.
* A [`bytes::Regex`](rust-regex-bytes) is always used. The only difference
between a `bytes::Regex` and a `Regex` from the crate root is that the former
can search `&[u8]` (i.e., arbitrary bytes) and the pattern is permitted to
match invalid UTF-8. For example, `(?-u:.)` will fail to compile using the
top-level `Regex` API, but will work just fine with `bytes::Regex`.
* We increase the [size limit] quite a bit so that the regex crate builds some
of the larger regexes. This does not result in any performance improvement. It
just increases a limit inside the regex compiler and permits bigger things to
be constructed. We specifically do _not_ increase the [DFA size limit], which
_could_ result in faster search times by virtue of giving the DFA more space to
store transitions, and thus less of a chance of clearing its cache or falling
back to a slower regex engine.

## Unicode

The regex crate is split into two APIs. The top-level `Regex` is for searching
`&str` and the `bytes::Regex` API is for searching `&[u8]`. The _only_
difference between `&str` and `&[u8]` is that `&str` is guaranteed to be valid
UTF-8. In general, `&str` is Rust's standard string type (with `String` being
the corresponding "owned" or heap allocating string buffer).

Since `&str` is guaranteed to be valid UTF-8 and slicing it at offsets in the
middle of a codepoint will result in a panic, the top-level `Regex` value
rejects any regex pattern that could match invalid UTF-8 (which includes
matching within a codepoint).

For `&[u8]` though, which can contain arbitrary bytes, there is no need for
such a restriction. And indeed, that's why we use the `bytes::Regex` API in
this benchmark. It is the maximally flexible API, and there is generally no
performance difference between the `&str` and `&[u8]` APIs. Indeed, they both
are just thin wrappers around an internal API that is just defined on `&[u8]`.

[rust-regex]: https://github.com/rust-lang/regex
[rust-regex-bytes]: https://docs.rs/regex/1.*/regex/bytes/index.html
[size limit]: https://docs.rs/regex/1.*/regex/bytes/struct.RegexBuilder.html#method.size_limit
[DFA size limit]: https://docs.rs/regex/latest/regex/bytes/struct.RegexBuilder.html#method.dfa_size_limit
