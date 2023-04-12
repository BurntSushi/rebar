This document describes the process for contributing to this barometer. We
mostly discuss the process for adding a new benchmark or a new regex engine,
but other changes are welcome as well. For small things, just submitting a pull
request is welcome. For bigger change requests or bug reports, please file an
issue. If you just have a question, then I encourage you to open a new
Discussion.

## Table of contents

* [Adding a new benchmark](#adding-a-new-benchmark)
* [Adding a new regex engine](#adding-a-new-regex-engine)

## Adding a new benchmark

**Summary:** Open a pull request with your new benchmark. If you're adding a
new _curated_ benchmark that will appear in the README, be prepared to defend
your submission as it needs to clear a high bar. Your PR comment should include
the results of running your benchmark, probably by showing the output of a
`rebar cmp` command.

Before getting into whether a benchmark _should_ be added, here is a brief
description of a benchmark's nuts and bolts. Namely, all benchmarks in rebar
have the following things:

* A name.
* A regex.
* A haystack.
* A list of engines with which to run the regex.
* An expected result. (Usually a count of matches.)

All benchmarks are defined in TOML files in the
[`benchmarks/definitions`](./benchmarks/definitions) directory. In
cases where the regex is too big to fit in a TOML file, they are in the
[`benchmarks/regexes`](./benchmarks/regexes) directory. Similarly, in cases
where the haystack is too big to fit in a TOML file (which is most of the
time), they are in the [`benchmarks/haystacks`](./benchmarks/haystacks)
directory.

For more information about how the `benchmarks` directory is structured,
including a complete description of the supported TOML format, please see
the [`FORMAT`](FORMAT.md) document. With that said, it is likely simple
enough that you can look at existing definitions, copy one of them and then
work from there.

The question for whether a benchmark *should* be added or not is a tricky one.
Here are two guiding principles:

* Adding a new benchmark to
[`benchmarks/definitions/curated`](./benchmarks/definitions/curated) needs
to clear a high bar. These are the benchmarks that make up the public face
of the barometer, and are intended to give a broad overview of the
performance of various regex engines.
* Adding a new benchmark elsewhere only needs to clear a much lower bar. At
this point in time, my bar for it is "it's interesting in some way." Ideally
it wouldn't be duplicative of another benchmark, but in practice there are so
many benchmarks and determining whether any two actually overlap is actually
quite difficult.

Either way, the process for submitting a new benchmark should be to open a PR
once you've defined your benchmark and successfully collected measurements. The
PR should include the full benchmark definition so that others can checkout
your branch and run the benchmark. Ideally, a benchmark will reuse an existing
haystack, but new ones can be added. We just need to be careful not to add too
many, as haystacks tend to be large and we don't want to bloat the repository.

To run your new benchmark and present the results in the PR,
you can use rebar. Let's say you added a new TOML file at
`benchmarks/definitions/wild/my-use-case.toml`. And in it, you defined this
benchmark:

```toml
[[bench]]
model = 'grep-captures'
name = 'shebang'
regex = '^(?P<spaces>\s*)#!(?P<directive>.*)'
unicode = true
haystack = { path = 'wild/cpython-226484e4.py' }
count = 282
engines = [
  'rust/regex',
  're2',
  'pcre2',
  'python/re',
]
```

Once you've created that file, you're now ready to test that the results
reported by each engine match what you expect:

```
$ rebar measure -f wild/my-use-case/shebang --test
wild/my-use-case/shebang,grep-captures,rust/regex,1.7.2,OK
wild/my-use-case/shebang,grep-captures,re2,2023-03-01,OK
wild/my-use-case/shebang,grep-captures,pcre2,10.42 2022-12-11,OK
Traceback (most recent call last):
  File "/home/andrew/code/rust/rebar/engines/python/main.py", line 429, in <module>
    results = model_grep_captures(config)
  File "/home/andrew/code/rust/rebar/engines/python/main.py", line 244, in model_grep_captures
    h = c.get_haystack()
  File "/home/andrew/code/rust/rebar/engines/python/main.py", line 85, in get_haystack
    return self.haystack.decode('utf-8')
UnicodeDecodeError: 'utf-8' codec can't decode byte 0xf6 in position 10247181: invalid start byte
wild/my-use-case/shebang,grep-captures,python/re,3.10.9,failed to run command for 'python/re'
some benchmarks failed
```

Ah! One of the benchmarks failed. This is because `python/re` only supports
searching valid UTF-8 when `unicode = true`. You can either set `unicode =
false`, or force the haystack to be converted to UTF-8 lossily before taking
measurements:

```toml
haystack = { path = 'wild/cpython-226484e4.py', utf8-lossy = true }
```

Now verification should succeed:

```
$ rebar measure -f wild/my-use-case/shebang --test
wild/my-use-case/shebang,grep-captures,rust/regex,1.7.2,OK
wild/my-use-case/shebang,grep-captures,re2,2023-03-01,OK
wild/my-use-case/shebang,grep-captures,pcre2,10.42 2022-12-11,OK
wild/my-use-case/shebang,grep-captures,python/re,3.10.9,OK
```

You're now ready to collect results. Results are reported on stdout. I usually
capture them in a file with `tee` so that I can see the results as they're
printed as well:

```
$ rebar measure -f wild/my-use-case/shebang | tee results.csv
name,model,rebar_version,engine,engine_version,err,haystack_len,iters,total,median,mad,mean,stddev,min,max
wild/my-use-case/shebang,grep-captures,0.0.1 (rev 1735337eec),rust/regex,1.7.2,,32514634,26,4.66s,118.56ms,11.04us,118.04ms,1.31ms,115.77ms,119.84ms
wild/my-use-case/shebang,grep-captures,0.0.1 (rev 1735337eec),re2,2023-03-01,,32514634,82,4.61s,37.04ms,3.21us,37.03ms,90.54us,36.85ms,37.30ms
wild/my-use-case/shebang,grep-captures,0.0.1 (rev 1735337eec),pcre2,10.42 2022-12-11,,32514634,83,4.56s,36.24ms,0.00ns,36.27ms,237.40us,35.78ms,36.86ms
wild/my-use-case/shebang,grep-captures,0.0.1 (rev 1735337eec),python/re,3.10.9,,32514634,7,5.06s,443.36ms,0.00ns,443.19ms,1.72ms,440.44ms,445.42ms
```

Ah, but unless you look really carefully, these results are not particularly
easy to interpret. Instead, we can ask rebar to do a comparison for us:

```
$ rebar cmp results.csv
benchmark                 pcre2               python/re           re2                 rust/regex
---------                 -----               ---------           ---                 ----------
wild/my-use-case/shebang  855.6 MB/s (1.00x)  69.9 MB/s (12.23x)  837.2 MB/s (1.02x)  261.5 MB/s (3.27x)
```

For your PR, include `results.csv` and the output of `rebar cmp` above in your
comment. That should make the benchmark and its results easier to review.

## Adding a new regex engine

**Summary:** Write a program that accepts the [KLV](KLV.md) format on stdin,
and prints CSV data consisting of of duration and count samples on stdout.
Put the program in a new directory `engines/<name>`, add an entry for it to
[`benchmarks/engines.toml`](benchmarks/engines.toml), build it with `rebar
build <name>` and add it to as many benchmark definitions as possible in
[`benchmarks/definitions/test`](benchmarks/definitions/test). Add it to other
relevant definitions as appropriate. Test it with `rebar measure -e <name>
--test`. Finally, submit a PR with rationale for why the regex engine should be
included.

Adding a new regex engine to this barometer generally requires doing the
following things:

1. Making a case for why the regex engine should be included.
2. Writing a program that accepts benchmark executions in the [KLV](KLV.md)
format on stdin, executes the indicated benchmark model repeatedly, and prints
timings and verification details for each execution.
3. Add the regex engine to existing benchmark definitions, and consider
whether new benchmarks should be added to account for the new regex engine.

The criteria for (1) are currently not so clear at the moment. On the one hand,
we shouldn't accept literally every regex engine. On the other hand, as long
as the maintenance burden for the engine is low and it has a sensible build
process, there shouldn't be much harm. For now, if the regex engine is actually
used by people in production _or_ has some particularly interesting property,
then I think it's probably fair game to include it. With that said, it does
need to have a sensible build process. I reserve the right to remove it in the
future if its build process causes too much pain.

### Build dependencies

In general, it is okay for a regex engine to fail to build. Some regex engines,
like `python/re` for example, won't build if `python` isn't available. This
means `python` is a build (and runtime in this case) dependency of this
barometer. But the barometer can function just fine when some of its regex
engines can't be built.

The dependencies for a regex engine _must_ be reasonable. The dependencies
should generally be installable via standard package managers. An example of
an unreasonable dependency would be a specific revision of llvm coupled with
patches that are included in this repository as part of the build process.

### Concrete steps for adding a new regex engine

The concrete steps to add a new regex engine are split into a few pieces,
because the steps vary somewhat depending on what you're doing.

#### Initial steps

1. Read through the [engines README](./engines/README.md) to get a high level
idea of how they work.
2. Create a new directory `engines/{regex-engine-name}`. If the regex engine
has its own unique name (e.g., "RE2" or "PCRE2" or "Hyperscan"), then use that.
Otherwise, if the regex engine is part of a language's standard library, then
use the language name. (If the _implementation_ of the language is relevant,
then the name should include some other disambiguating term.)
3. Choose whether the runner program will be written in Rust or not. Generally
speaking, if the regex engine is written in C, C++, Rust or some other language
that is easy to use from Rust via a C FFI at zero cost, then Rust should be
used as it eases the maintenance burden of the overall barometer. Otherwise,
a different language can be used. If you choose Rust, skip the next section.
Otherwise, mush on.

#### For non-Rust runner programs

1. There is no mandated structure for what goes in this directory. It just
needs to be a buildable program, and that program needs to accept [KLV](KLV.md)
on stdin. It should then collect samples by executing the benchmark repeatedly,
up to a certain time limit or a number of iterations (which ever is reached
first). Each sample is just the duration in the number of nanoseconds and a
single integer "count" corresponding to the output of the benchmark (which
varies based on the [model](MODELS.md) used). The output format is just
printing each sample on its own line, with the duration followed by the count,
separated by a comma. If you need help, consult another non-Rust runner
program. (I say non-Rust because the non-Rust programs tend to be well isolated
self-contained programs, where as the Rust programs---because there are many of
them---tend to have reusable components.)
2. Your program should be as self-contained as possible and use as little
(ideally none) dependencies as possible, other than the environment's standard
library. The runner program requirements are specifically simplistic to
support this. The only thing that's required is standard I/O, some light string
parsing and the ability to measure durations to nanosecond precision.
3. Skip the next section about Rust runner programs and move on to the section
about testing and finishing your runner program.

#### For Rust runner programs

If you're adding a new regex engine that is managed through a Rust runner
program, then some of the bits and bobs you need (such as parsing the
KLV format and collecting measurements) are already written for you.
With that said, this means there is a little more setup. When in doubt,
you can always reference one of the several other Rust runner programs.
[`engines/re2`](engines/re2) is a good one that shows FFI to a C++
library, where as [`engines/regress`](engines/regress) shows how to call a
written-in-Rust regex engine.

1. In the `engines/{name}` directory you just created, run
`echo 'fn main() {}' > main.rs` and `cargo init --bin`.
2. Add `engines/{name}` to the `exclude` array in [`Cargo.toml`](Cargo.toml).
3. Add `engines/{name}/Cargo.toml` to the array in
[`.vim/coc-settings.json`](.vim/coc-settings.json).
4. Edit your `engines/{name}/Cargo.toml` file to look roughly similar to
[`engines/regress/Cargo.toml`](engines/regress/Cargo.toml). In particular,
the name of the program, the `[[bin]]` section, and the `dependencies.klv`,
`dependencies.regexredux` and `dependencies.timer` sections.
5. You should be able to run `cargo build` in `engines/{name}` and get a
working program at `target/debug/main`.
6. Consult the [`engines/regress/main.rs`](engines/regress/main.rs) program
for how to use the `lexopt`, `anyhow`, `klv`, `regexredux` and `timer`
dependencies to compose a runner program.

In the likely event that you're trying to add a regex engine that
_isn't_ written in Rust, you'll need to create an FFI shim. This
guide won't cover how to do that in detail, but you should be able
to follow along with the [`engines/pcre2`](engines/pcre2) and
[`engines/re2`](engines/re2) examples. You'll also want to add a script like
[`scripts/update-re2`](scripts/update-re2) if it makes sense to vendor the
regex engine source itself. (If the regex engine is a standalone library with a
reasonable build process, then it probably does.)

#### Testing and finishing your runner program

At this point, you should be able to build your runner program and you feel
ready to test it.

1. You can test your runner program directly by using `rebar` to produce KLV
data from any benchmark. For example, `rebar klv curated/08-words/all-english
--max-time 3s --max-iters 10 | ./engines/go/main` will run the
`curated/08-words/all-english` benchmark at most 10 times (and up to 3 seconds)
using the `go/regexp` regex engine. Just swap out `./engines/go/main` with the
executable to your program to test it.
2. Add a new regex engine entry to
[`benchmarks/engines.toml`](benchmarks/engines.toml). This makes it
available as an engine that one can use inside benchmark definitions. See
the [FORMAT](FORMAT.md) document for what kind of things are supported. One
important bit here is that getting the version number *must* act as a receipt
that the regex engine has been built and can be run successfully in the current
environment.
3. Check that `rebar build -e <regex-engine-name>` works for your regex engine.
4. Add your regex engine to as many of the benchmark definitions in
[`benchmarks/definitions/test/`](benchmarks/definitions/test/) as
possible. If none are appropriate, then please open an issue discussing the
regex engine and why it should be added at all.
5. Test that everything works by running `rebar measure -f '^test/' -e
go/regexp --test`, but with `go/regexp` replaced with the name
of the regex engine you added to `engines.toml`. You should see output like
`test/model/count,go/regexp,count,1.20.1,OK` (among others).
6. Add the engine to benchmark definitions as appropriate.
7. Add a `engines/{name}/README.md` file explaining some of the choices made in
your runner program, and include an upstream link to the regex engine. See the
README files for other engines for examples.
8. Submit a pull request. Ensure that others are able to checkout your PR,
run `rebar build -e <name>` and are able to test it by running `rebar measure
-e <name> --test`. **CI must be able to run `rebar build` in its entirety
successfully.**
