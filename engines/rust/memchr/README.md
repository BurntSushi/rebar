This directory contains a Rust runner program for benchmarking the [`memchr`
crate][rust-memchr]. Specifically, it measures the substring search algorithm
implemented in the [`memchr::memmem`][memmem] submodule.

Like the `aho-corasick` crate, this crate is principally used for implementing
prefilters inside of [Rust's regex crate][rust-regex]. It implements a
number of different algorithms: a variant of ["generic SIMD"][genericsimd],
[Two-Way][twoway] and [Rabin-Karp][rabinkarp].

As with the [`rust/aho-corasick`](../aho-corasick/README.md) engine, this only
supports searching for a single literal. If the number of patterns given is not
equal to one, then the runner program will report an error. The runner program
also doesn't do any escaping, and always treats whatever pattern is given to it
as a literal. It is therefore up to the author of the benchmark definition to
ensure that only literals are given to this engine.

As with the `rust/aho-corasick` engines, this only implements the `compile`,
`count`, `count-spans` and `grep` benchmark models.

[rust-memchr]: https://github.com/BurntSushi/memchr
[memmem]: https://docs.rs/memchr/2.*/memchr/memmem/index.html
[rust-regex]: https://github.com/rust-lang/regex
[genericsimd]: http://0x80.pl/articles/simd-strfind.html
[twoway]: https://en.wikipedia.org/wiki/Two-way_string-matching_algorithm
[rabinkarp]: https://en.wikipedia.org/wiki/Rabin%E2%80%93Karp_algorithm
