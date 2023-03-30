This directory contains a Rust runner program for benchmarking [ICU
Regular Expressions][icu-regex]. It uses [ICU4C's regex API][uregex-api].

This program makes the following choices:

* It will error if given a haystack that contains invalid UTF-8. Namely, as far
as I can tell, there is no way to use ICU's regex engine on arbitrary bytes.
Its API seems to suggest that it is only possible to run it on sequences of
UTF-16 code units.

## Unicode

For obvious reasons, ICU has excellent Unicode support. It is also impossible
to disable Unicode mode, which makes sense, because its Unicode features are
probably the reason why you would use ICU's regex engine in the first place.

Note that we don't currently enable its `UREGEX_UWORD` option under any
circumstances, and instead let `\b` just be Unicode-aware in the same way that
most other regex engines are. This is because ICU is probably unparalleled
here, although it might be nice to add a benchmark for it.

[icu-regex]: https://unicode-org.github.io/icu/userguide/strings/regexp.html
[uregex-api]: https://unicode-org.github.io/icu-docs/apidoc/released/icu4c/uregex_8h.html
