This directory contains a Java runner program for benchmarking [Java regular
expressions][java-regex]. Specifically, this is for the `java.util.regex`
package in the JDK. Java's regex engine is principally backtracking based.

This program otherwise makes the following choices:

* It will throw an exception if given a haystack that contains invalid UTF-8.
Namely, as far as I can tell, there is no way to use Java's regex engine on
arbitrary bytes. Its API seems to suggest that it is only possible to run it on
sequences of UTF-16 code units.

## Compilation & flawed measurements

While I could not find any explicit code in `java.util.regex.Pattern` that
caches compilation of regexes, it turns out that as compilation is repeated,
it gets progressively faster by orders of magnitude. To a point where it is
implausibly fast.

This is almost certainly because of "JIT warming." That is, since regex
compilation is repeated many times without anything changing, it's possible
that most of it is actually being skipped after the JIT is "warmed" up enough.

For this reason, I've not included Java in any curated compilation benchmarks.
I am not totally certain that this is the right decision, but it certainly
seems like the compilation model used by rebar is not particularly realistic
for Java programs specifically.

One wonders how a similar JIT warming process applies to search. And in
particular, our measurement process might be completely flawed here. Our models
generally run benchmarks repeatedly without actually changing anything, where
as in the real world, the haystacks are very likely to change. So if Java's JIT
can detect that the regex search has invariant inputs, then in theory, it could
deduce that its output is invariant too. But whether this is actually plausible
or not isn't totally clear. (Especially speaking as someone who isn't an expert
in JITs.)

Anyway, I very much welcome feedback from someone who understands more about
benchmarking Java programs than I do (which is very little).

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
