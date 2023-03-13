This directory contains a Rust runner program for benchmarking the [regress]
regex engine. The goal of this regex engine is to implement [EcmaScript regex
syntax and semantics][ecma].

The following decisions are made by this runner program:

* The `regress` regex API doesn't permit searching anything other than a Rust
`&str`, which implies the API only supports searching valid UTF-8. Therefore,
this runner program always returns an error when the haystack is invalid UTF-8.
* The `regress` API does have a Unicode mode that can be toggled on or off, but
it looks like it only currently impacts surface level syntax support.

## No support for inline flags

Most regex engines support syntax like `(?s:.)` to enable flags for a
sub-expression of the regex. In the case of `(?s:.)`, it enables "dot all"
mode in most regex engines such that `.` matches any character instead of its
typical default of matching any character except for newline terminators.

regress, likely because of its adherence to EcmaScript, does not support these
inline flags. This means it cannot be used in some benchmarks that make use of
these flags. In some cases, we try to rewrite the regex in order to avoid the
use of these flags, but we don't expend the effort in every case.

Arguably, rebar could be expanded to push some of these flags into the
benchmark definition itself, similar to `unicode` and `case-insensitive`.
Although, that still doesn't resolve everything, because sometimes flags are
only enabled for parts of the regex and not all of it.

## Unicode

Similar to Go's regexp engine, `regress` has support for Unicode mode in some
aspects but not in others. For example, `.` and `[^a]` match entire codepoints
instead of individual bytes and case insensitivity is implemented by taking
Unicode case folding rules into account. Also like Go, though, the classes
`\w`, `\d` and `\s` are always limited to their ASCII definitions. Unlike
Go, as of `regress 0.5.0`, there is now a way of disabling Unicode mode. But
that Unicode mode seems to only impact surface level syntax, for example, by
rejecting invalid bounded repetition syntax.

One other thing worth mentioning is that `regress` has some support for Unicode
character class syntax, but tends to diverge from other regex engines in this
barometer. For example, `regress` supports `\p{Letter}` but not `\pL`, while
PCRE2, RE2 and Go's `regexp` package all support `\pL` but not `\p{Letter}`.

(It's likely that some or all of this support is dictated by ECMAScript, but
I didn't spend the time to dig into that.)

[regress]: https://github.com/ridiculousfish/regress
[ecma]: https://tc39.es/ecma262/#sec-regexp-regular-expression-objects
