This directory contains a D-lang runner program for benchmarking [D regular
expressions][phobos-std-regex]. Specifically, this is for the `std.regex`
package in the standard library. D's regex engine is a hybrd that makes use
of both finite automata and backtracking strategies. Namely, by generally
following ECMAScript, some features (such as look-around) make use of
backtracking in a way that makes the regex engine susceptible to catastrophic
backtracking. Interestingly, I could find zero mention about the time and space
complexity of D's regex engine in its documentation.

Like other engines such as `perl` and `javascript/v8`, we exclude D's regex
engine from compile-time benchmarks because it appears that [some caching] is
used.

## Unicode

D's regex engine enjoys good Unicode support by supporting nearly all of
[UTS#18].

Disabling unicode mode is not supported.

[phobos-std-regex]: https://dlang.org/phobos/std_regex.html
[UTS#18]: http://unicode.org/reports/tr18/
[some caching]: https://github.com/dlang/phobos/blob/d945686a4ff7d9fda0e2ee8d2ee201b66be2a287/std/regex/package.d#L389-L423
