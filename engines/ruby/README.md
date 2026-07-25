This directory contains a Ruby runner program for benchmarking Ruby's built-in
regex engine, which is based on [Onigmo][onigmo] (a fork of Oniguruma).

The Ruby runner program makes the following decisions:

* Only one pattern is supported.
* When Unicode mode is enabled, Ruby's regex engine already handles Unicode
  by default, so no special flags are needed.
* When case-insensitive mode is requested, the `i` flag is used.
* Ruby regexes can handle invalid UTF-8 in strings, similar to Python's
  behavior with bytes vs strings.

## Unicode

Ruby's regex engine (Onigmo) has Unicode support, but with important limitations:

* Unicode property escapes like `\p{Letter}` and `\p{Nd}` work correctly
* **However**, the common character classes are ASCII-only:
  - `\w` only matches `[A-Za-z0-9_]`, not Unicode word characters
  - `\b` word boundaries don't work with non-ASCII text (e.g., Cyrillic, Chinese)
  - `\d` only matches `[0-9]`, not Unicode digits (e.g., Arabic-Indic numerals)
* The engine can handle invalid UTF-8 sequences in strings when using binary encoding

For Unicode text, use `\p{L}` instead of `\w` and `\p{Nd}` instead of `\d`.

## Version

This runner is tested with Ruby 3.x but should work with Ruby 2.x as well.
The version reported is Ruby's version, which effectively indicates the version
of Onigmo being used.

## Known Limitations

1. **No multi-pattern support**: Ruby cannot run benchmarks with `per-line = "pattern"`
   configuration, as it lacks APIs for searching multiple patterns simultaneously.

2. **ASCII-only defaults**: This causes count mismatches in Unicode benchmarks that use
   `\w`, `\b`, or `\d`. This is documented Onigmo behavior, not a bug.

See the [Onigmo documentation](https://github.com/k-takata/Onigmo/blob/master/doc/RE) 
for more details about character class behavior.

[onigmo]: https://github.com/k-takata/Onigmo