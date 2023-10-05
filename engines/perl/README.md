This directory contains a Perl runner program for benchmarking [Perl's
regex engine][perlre]. Perl's regex engine uses backtracking.

This runner program makes a few choices worth highlighting:

* Regexes are specifically compiled using `my $re = qr/.../;`, but in order to
do an iterative search, they seemingly need to be reformulated in regex syntax
like `/$re/g`, since it seems the `g` flag cannot be used in combination with
`qr/.../` syntax. (Which makes sense, since the `g` flag is likely connected to
search time state, where as `qr/.../` is really about just getting the regex
into a compiled form.) This also makes sense conceptually, but using the syntax
`/$re/g` to execute a search kind of makes it _look_ like the regex is being
re-compiled. We do _not_ want to measure regex compilation during a search if
we can help it. So the question is: does `/$re/g` re-compile `$re` if `$re` is
already a compiled regex object? I could not find the answer to this question.
* The `grep` and `grep-captures` modles use a regex to iterate over lines. This
is somewhat odd, but appears idiomatic. It's also probably pretty likely to
be the fastest way to do such a thing in Perl, although I don't know for sure.
If there's a better and/or more idiomatic approach, I'd be happy for a PR where
we can discuss it.

## Compilation

While I don't know for sure, my _suspicion_ is that the compilation benchmark
model is not implemented correctly. My guess is that Perl regexes are being
cached, and so this runner program doesn't actually measure what it's supposed
to measure. After a cursory search, I could neither confirm nor deny this
hypothesis and could find no way to clear any cache that Perl might be using.
(Python has `re.purge()`, which we use for exactly this purpose.)

UPDATE: Thanks to Nick Johnston, it can be confirmed that my suspicion above is
correct. Namely, that Perl has some caching mechanism to avoid re-compiling
the same regex pattern in at least some cases. One can confirm this via the
following command:

```
$ perl -Mre=debug -E '
    my $re = qr/(snake|crocodile)/;
    my $haystack = "snake on a crocodile";
    say $1 while $haystack =~ /$re/g;
'
```

Specifically, even though `/$re/g` is used twice, the regex itself is only
compiled once.

There is still yet no known way to avoid this caching behavior, and thus Perl's
regex engine is not included in any of the curated compilation benchmarks.

## Unicode

Perl's documentation boasts quite impressive support for Unicode, and it can be
toggled via the `a` and `u` flags (among some others). Unfortunately, actually
getting them to work is quite a challenge, and I'm still not quite sure that
this program does it correctly. Review on this point would be appreciated.

But, what I do believe is true is that when Unicode is enabled in the benchmark
definition, then things like `\w`/`\s`/`\d`, `\b` and case insensitivity are
all Unicode-aware. And when Unicode is disabled, then all of those things are
limited to their ASCII only interpretations. Which is perfect and ultimately
works similarly to Rust's regex engine. (The main problem is that whether any
of this works correctly is up to certain properties of the strings themselves,
and most of it is pretty silent.)

## Warning

This is the first real Perl program I've ever written. It is probably not
idiomatic. I'm open to PRs making it more idiomatic, but I'd prefer if it not
become even more cryptic than it already is.

[perlre]: https://perldoc.perl.org/perlre
