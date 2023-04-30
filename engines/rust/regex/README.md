This directory contains a Rust runner program for benchmarking [Rust's regex
crate][rust-regex]. The `regex` crate is authored by the same person who
created this barometer, and the barometer's early motivation was for tracking
and improving the performance of the regex crate.

This runner program makes the following decisions:

* Multiple patterns are supported by using the lower level `meta::Regex` API
from `regex-automata` instead of the APIs from the actual `regex` crate. (See
the section below for why we do this.)
* UTF-8 mode is always disabled. This just means that matches that split
a codepoint may be reported and generally doesn't have much impact on
performance.
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
[size limit]: https://docs.rs/regex/1.*/regex/bytes/struct.RegexBuilder.html#method.size_limit
[DFA size limit]: https://docs.rs/regex/latest/regex/bytes/struct.RegexBuilder.html#method.dfa_size_limit

## Using `regex-automata` instead of `regex`

This runner program actually uses the `regex-automata` crate instead of the
`regex` crate. The reasons for doing this are a little ham-fisted, but roughly
as follows:

1. The `regex` crate is quite literally a API wrapper around `regex-automata`'s
`meta::Regex` API. The `regex` crate doesn't do any added processing. This
design was specifically used so that folks could drop down to the lower level
API without much fuss.
2. The `regex` crate API doesn't have good support for multi-pattern searching,
but the `meta::Regex` API in `regex-automata` supports it natively. The `regex`
crate _does_ have a `RegexSet` API, but at the time of writing, it's limited
to determining whether zero or more regexes match anywhere in a haystack. It
doesn't report match offsets or capture groups. `meta::Regex` does. Since some
benchmarks want multi-pattern support (to square off against Hyperscan), it is
really confusing to use, say, both a `rust/regex` and a `rust/regex/meta`
engine.
3. In fairness to (2) above, I did actually try to maintain two distinct
engines in the benchmarks. That is, `rust/regex` and `rust/regex/meta`.
Everywhere `rust/regex` was used, `rust/regex/meta` was also used. But
`rust/regex/meta` was _also_ used in the multi-pattern benchmarks, which
`rust/regex` could not participate in because of API limitations. The problem
with this approach is that it was really confusing to have both of these
engines in the results. It's essentially inside baseball. Moreover, it made
comparing Rust's regex crate with other engines more difficult than necessary,
and in some cases, impossible without awkward changes to the tooling that would
permit engine names to alias in certain cases.
4. Speed is actually the same in practice between `rust/regex` and
`rust/regex/meta` for all benchmarks in which both can participate. I actually
checked this, but it also _should_ be true since a `regex::Regex` is quite
literally a thin wrapper around `meta::Regex`.

What I did to test (4) above was have both `rust/regex` and `rust/regex/meta`
engines. I ran many benchmarks on both (i.e., all single pattern benchmarks). I
then compared the results. If my thesis was correct, I should observe largely
similar timings. We can check at a high level via `rebar rank` for search
benchmarks:

```
$ rebar rank record/curated/2023-04-29/*.csv --intersection -M compile -e '^rust/regex(/meta)?$'
Engine           Version  Geometric mean of speed ratios  Benchmark count
------           -------  ------------------------------  ---------------
rust/regex       1.8.1    1.01                            32
rust/regex/meta  0.3.0    1.01                            32
```

And for compilation benchmarks:

```
$ rebar rank record/curated/2023-04-29/*.csv --intersection -m compile -e '^rust/regex(/meta)?$'
Engine           Version  Geometric mean of speed ratios  Benchmark count
------           -------  ------------------------------  ---------------
rust/regex/meta  0.3.0    1.01                            12
rust/regex       1.8.1    1.01                            12
```

I also looked at individual benchmark results to ensure the geometric mean
wasn't giving a false impression. It wasn't. You can also look at this data
yourself too by using the data in the above commands. (It's committed to the
repository.) Just change `rebar rank` in the above commands to `rebar cmp`.

Overall, I do personally think that not actually using the `regex` crate in
benchmarks that purport to measure the `regex` crate is quite suspicious, but
I did feel like this path was the _least_ confusing. If, in the future, the
`regex` crate gains a more expressive API (I'd love if `RegexSet` could at
least become that in a backwards compatible manner, but it's tricky) that
supports multi-pattern searches, then I'll move the runner program over to the
real `regex` crate APIs.

I'm open to other ideas if folks have them.
