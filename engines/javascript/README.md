This directory contains a Javascript runner program for benchmarking [Javascript regular
expressions][mdn-regexp]. This currently only supports the [irregexp] engine
inside of v8. (Which is used by Google Chrome, Firefox and NodeJS.)

Note that v8, as of somewhat recently, does contain an experimental
[non-backtracking regex engine][nobacktrack]. It would be cool to measure that,
but it's not clear what the best approach is there.

This program otherwise makes the following choices:

* It will throw an exception if given a haystack that contains invalid UTF-8.
Namely, as far as I can tell, there is no way to use Javascript's regex engine
on arbitrary bytes. Its API seems to suggest that it is only possible to run it
on Javascript strings, which I understand to be sequences of UTF-16 code units.

## Compilation & flawed measurements

Like the `java/hotspot` regex engine, it looks like the JIT is potentially
caching regex compilation in some way. Basically, after a number of iterations,
regex compilation becomes stupidly fast. This is probably not a good model of
the real world, and so, `javascript/v8` is not included in any of the compilation
benchmarks.

See also the discussion in [the Java runner program's](../java) README. For
anyone with more experience with v8 and irregexp, I would welcome feedback.

## Unicode

Javascript's regex engine does okay with Unicode support, but only when Unicode
mode is enabled, which is not the default. The biggest difference between
non-Unicode mode and Unicode mode is probably that the former uses UTF-16
code units as the fundamental atom of matching, where as the latter uses
full codepoints as the fundamental atom of matching. This has the effect
where non-Unicode tends to not support codepoints outside of the BMP (basic
multi-lingual plane).

[mdn-regexp]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/RegExp
[irregexp]: https://blog.chromium.org/2009/02/irregexp-google-chromes-new-regexp.html
[nobacktrack]: https://v8.dev/blog/non-backtracking-regexp
