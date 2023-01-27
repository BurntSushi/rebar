This directory contains a Go runner program for benchmarking [Go's standard
library `regexp` package][go-regexp]. This runner program (in addition to
Python's `re` standard library module) were early motivators for rebar's
sub-process architecture, as neither Go's nor Python's regex engine can be
_easily_ exposed over a C API.

Go's regexp package is directly descended from [RE2], and in particular
guarantees linear time searches (with respect to the size of the haystack) for
all inputs.

The Go runner program makes the following decisions:

* Only one pattern is supported.
* Case insensitive mode is implemented by wrapping the pattern given in
`(?i:<PATTERN>)`.
* Go is assumed to always have Unicode mode enabled, but also works when
Unicode mode is disabled. (See below.)

## Unicode

Go does not have an explicit Unicode mode, _but_ it does support Unicode
features. For example, `.` and `[^\n]` both match entire codepoints in valid
UTF-8. Go also has limited support for some Unicode classes such as general
categories like `\pL` and scripts like `\p{Greek}`. Moreover, its case
insensitivity functionality is always Unicode aware, and there is no way to
disable that.

Also, Go's "Perl" character classes (`\w`, `\s` and `\d`) are always defined
to be limited to ASCII and are never Unicode aware.

Another interesting note is that Go's regex engine is able to search invalid
UTF-8, but it will treat any invalid UTF-8 bytes as if they were `U+FFFD`. So
a Unicode aware regex like `.` will actually match through invalid UTF-8. The
benchmark `unicode/compile/match-every-line` demonstrates how Go differs from
other regex engines (including RE2) in this regard. For motivation as to why
Go's regex engine behaves this way, see [this commit][go-regexp-utf8].

In essence, all of the above points put Go in a sort of tweener state with
respect to Unicode. It supports Unicode in some respects, but not in others,
and there's no way to toggle the behavior. You get what you get. Because of
this, the runner program effectively ignores rebar's `unicode` option. This
essentially means that the onus is on the author of the benchmark to take
into account Go's behavior. This is of course one of the many reasons why
rebar verifies results, because it can otherwise be quite easy to overlook
differences in the semantics of `\w`.

[go-regexp]: https://pkg.go.dev/regexp
[RE2]: https://github.com/google/re2
[go-regexp-utf8]: https://github.com/golang/go/commit/702e33717486cb8331db17304f2369ef641da61f
