This directory contains a Rust runner program for benchmarking [RE2]. RE2 was
written by Russ Cox and came out of his [series of articles on Implementing
Regular Expressions][rsc-regexp]. The big idea behind RE2 was to provide a
subset of Perl regular expression features while using finite automata to
guarantee that searches execute in time linear to the length of the haystack.

RE2 has several regex engine descendents, notably, [Go's standard library
regexp package][go-regexp] and [Rust's regex crate][rust-regex]. All three
libraries have a similar implementation strategy. That is, each contains a
number of internal regex engines, and for each search, one (or more) of those
engines is selected based on various criteria to service a request. In most
cases, the criteria considered is performance.

This Rust runner program makes the following decisions:

* Only one pattern is supported. (We do not benchmark RE2's "regex set"
functionality.)
* When Unicode mode is disabled, then we compile regexes using RE2's
`EncodingLatin1` option. Note that like Go's regexp package, the `\w`, `\d`
and `\s` character classes always use their ASCII definition.

[RE2]: https://github.com/google/re2
[rsc-regexp]: https://swtch.com/~rsc/regexp/
[go-regexp]: https://pkg.go.dev/regexp
[rust-regex]: https://github.com/rust-lang/regex
