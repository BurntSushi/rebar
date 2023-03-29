This directory contains a Java runner program for benchmarking [Java regular
expressions][java-regex]. Specifically, this is for the `java.util.regex`
package in the JDK. Java's regex engine is principally backtracking based.

This program otherwise makes the following choices:

* It will throw an exception if given a haystack that contains invalid UTF-8.
Namely, as far as I can tell, there is no way to use Java's regex engine on
arbitrary bytes. Its API seems to suggest that it is only possible to run it on
sequences of UTF-16 code units.

## Unicode

Java's regex engine has pretty good Unicode support. It does Unicode case
folding for case insensitive matching, `\w`/`\s`/`\d` are all Unicode-aware.
`\b` is also Unicode-aware. And things like `.` match entire codepoints.

Java also supports disabling Unicode mode, with independent toggles for
Unicode case folding and Unicode character classes. The `unicode` toggle
in rebar's benchmark definition toggles both of Java's Unicode options in
tandem.

## Warning

This is the first real Java program I've ever written in about a couple
decades. (With the last one being for my AP Computer Science class in high
school.) It is probably not idiomatic. I'm open to PRs making it more idiomatic
or other improvements.

[java-regex]: https://docs.oracle.com/javase/7/docs/api/java/util/regex/Pattern.html
