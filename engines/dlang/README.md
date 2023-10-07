This directory contains a D-lang runner program for benchmarking [D regular 
expressions][phobos-std-regex]. Specifically, this is for the `std.regex`
package in the standard library. D's regex engine is principally backtracking based.

## Unicode

While D is unicode aware, it's support is not fully complete. In some cases `\w` and `\b`
dont support all unicode classes. `.` always matches entire codepoints.

Disabling unicode mode is not supported at all.

[phobos-std-regex]: https://dlang.org/phobos/std_regex.html