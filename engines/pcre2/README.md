This directory contains a Rust runner program for benchmarking
[PCRE2][pcre2-github]. This includes a distinct engine each for both PCRE2's
standard "interpreter" engine and its JIT engine. We do not benchmark PCRE2's
so-called ["DFA" engine][pcre2-dfa] because it doesn't seem worth the effort.

PCRE2 is short for "Perl compatible regular expressions." In effect, PCRE2
makes many of Perl's advanced regex features available as a C library. It is
also notable for its JIT, for which the [sljit] project exists.

The runner program makes the following decisions:

* Only one pattern is supported.
* Whenever Unicode is requested, both `PCRE2_UCP` and `PCRE2_MATCH_INVALID_UTF`
are enabled. Notably, the latter also forces `PCRE2_UTF` to be set, but
permits searching haystacks with invalid UTF-8. (Otherwise, without
`PCRE2_MATCH_INVALID_UTF`, enabling Unicode mode while searching invalid UTF-8
lead to undefined behavior.)
* `pcre2_match` is always used. Namely, `pcre2_jit_match` is never used.
This is because the man page states that it bypasses "sanity checks," and
that it could lead to undefined behavior. But it's not particularly explicit
about what needs to be passed to avoid undefined behavior. Moreover, since
`pcre2_jit_match` presumably bypasses a constant number of sanity checks,
this likely only impacts latency oriented benchmarks where throughput is not
measured. This of course matters, but many benchmarks in rebar are throughput
oriented and are thus unlikely to be impacted by the use of `pcre2_jit_match`.
(I could be convinced to switch over to `pcre2_jit_match`, but I think I'd also
want to see some evidence that this is what serious projects actually use in
practice.)
* When possible, this creates space for capturing groups beyond the match
position only when it is needed. So for example, the `count`, `count-spans`
and `grep` model implementations should never need to track any capturing
groups other than the one that represents the overall match.
* Since PCRE2 can return an error at search time, _all_ search calls are
checked that an error did not occur.
* PCRE2's match limit is set to the maximum (`u32::MAX`) possible value. This
makes it so we can meaningfully test cases that cause it to catastrophically
backtrack instead of just having it give up and not report a result at all. It
makes sense to do this because folks might increase the limit in the wild, or
the limit might not be sufficient to detect all cases of exponential search
times. So it's important to explore what happens when the worst happens.

## Unicode

PCRE2's Unicode mode---once the right flags are set---works quite similarly to
Rust's regex crate Unicode mode. That is, it is perfectly possible to compile
regular expressions with Unicode mode enabled while simultaneously searching
haystacks that may contain invalid UTF-8. Invalid UTF-8 just simply does not
match Unicode-aware regex constructions. (This is also how RE2 works.)

[pcre2-github]: https://github.com/PCRE2Project/pcre2
[pcre2-dfa]: https://pcre2project.github.io/pcre2/doc/html/pcre2matching.html#SEC4
[sljit]: https://github.com/zherczeg/sljit
