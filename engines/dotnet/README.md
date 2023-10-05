This directory contains a C# runner program for benchmarking [.NET regular
expressions][dotnet-regex].

.NET's regex engine is principally backtracking based, with an option for JIT
compilation. As of .NET 7, there is also an option to use a non-backtracking
engine. In total, this results in three regex engines that this runner program
can execute: `dotnet`, `dotnet/compiled` and `dotnet/nobacktrack`. In general,
we only measure the latter two, since it's expected that if one cares about
performance, then they'll avoid using the pure interpreter based regex engine.
(.NET also provides regexes that compile to C# source code. In theory, we
could measure those too, but it would require writing a program to build .NET
programs, which is perhaps more trouble than it's worth.)

This program otherwise makes the following choices:

* It will throw an exception if given a haystack that contains invalid UTF-8.
Namely, as far as I can tell, there is no way to use .NET's regex engine on
arbitrary bytes. Its API seems to suggest that it is only possible to run it on
sequences of UTF-16 code units.
* In general, Unicode mode is always assumed to be enabled since there doesn't
appear to be any way to disable it.
* We use `RegexOptions.CultureInvariant` to avoid having the regex engine do
any extra work for custom Unicode tailoring. Most regex engines don't support
this kind of thing, so we don't require .NET to do it.

## Unicode

.NET's regex engine has pretty good Unicode support, but it's limited to
matching UTF-16 code units and not Unicode codepoints. It does Unicode case
folding for case insensitive matching, `\w`/`\s`/`\d` are all Unicode-aware.
`\b` is also Unicode-aware. Unfortunately, due to the atom of matching being
a UTF-16 code unit, things like `\w` and even `.` will only match codepoints
limited to the basic multi-lingual plane. Any codepoint requiring the encoding
of two UTF-16 code units won't be matched.

As far as I can tell, there is no obvious way to disable Unicode features, so
it is always enabled. Because other regex engines also don't permit toggling
Unicode mode (or do, but only to a certain extent), this generally means that
we just need to be careful in our benchmark definitions. For example, we use
`[0-9A-Za-z_]` instead of `\w` when we are specifically only interested in an
ASCII search and not a full Unicode search.

In some cases though, we just have to "live" with the fact that certain things
are Unicode-aware. For example, `\b` in .NET's regex is always Unicode aware.

## Warning

This is the first real .NET program I've ever written. It is probably not
idiomatic. I'm open to PRs making it more idiomatic or other improvements.

[dotnet-regex]: https://learn.microsoft.com/en-us/dotnet/standard/base-types/regular-expressions
