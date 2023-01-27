This directory contains a Python runner program for benchmarking both [Python's
standard library `re` module][python-re] and the third party [`regex`][python-regex]
module.

The Python runner program takes advantage of the fact that the API of the third
party `regex` module is drop-in compatible with the `re` module. So all it does
is `import regex as re` instead of `import re` when one is benchmarking it
for the `regex` module.

Otherwise, the runner program makes the following decisions:

* Only one pattern is supported.
* When the `regex` module is used, `regex.DEFAULT_VERSION` is set to
`regex.VERSION1`. This is done because it has better Unicode support, and is
presumably the more interesting thing to measure.
* When Unicode mode is enabled, the runner program reports an error if the
haystack is invalid UTF-8. (See below.)

## Unicode

Both the `re` and `regex` modules have two different types of regexes: ASCII
regexes and Unicode regexes. ASCII regexes can be built from either byte
string patterns or Unicode string patterns, and an ASCII regex can only search
haystacks corresponding to the same type of its pattern. Conversely, Unicode
regexes can only be built from Unicode strings, and Unicode regexes can only
search Unicode strings.

Since Unicode strings cannot be non-lossily constructed from invalid UTF-8,
it follows that neither the `re` nor the `regex` modules can search invalid
UTF-8 while Unicode mode is enabled. Thus, this is why enabling Unicode mode
for these regex engines requires the haystack to be valid UTF-8.

The "use bytes or Unicode" split actually infects pretty much everything about
the regex APIs in Python-land. Once you make your choice about the type of your
pattern, everything you then pass into the regex engine must have the same
type.

[python-re]: https://docs.python.org/3/library/re.html
[python-regex]: https://github.com/mrabarnett/mrab-regex
