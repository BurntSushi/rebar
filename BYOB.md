This document describes how to "bring your own benchmarks." In particular, one
intentional design decision for `rebar` is that it doesn't have any specific
knowledge about benchmarks, models or even the engines themselves. Namely:

* Benchmarks are just TOML tables that include information like the regex to
compile and the haystack to search.
* Models are just a label given to a particular work-load. `rebar` doesn't care
what the model is. The intent is for every runner program to implement each
model in an apples-to-apples manner.
* Engines are also just TOML tables that attach a label (a regex engine name)
to a set of commands for `rebar` to run.

All of this information is contained within the [`benchmarks`](./benchmarks/)
directory of this repository. But you can create your own `benchmarks`
directory. And it can live anywhere. You just need to pass the `-d/--dir`
flag to any rebar commands that need to read the benchmark definitions. (For
example, `rebar measure` and `rebar report`.)

This document will demonstrate this by showing how to build our own `memmem`
benchmark suite. (`memmem` is a POSIX routine for substring search. That is,
given a needle and a haystack, it returns the first occurrence of the needle
in the haystack, if any.) We'll create our own benchmark definitions, create
our own runner programs, create our own models and define our own engines. In
this exploration, we will do a bake off between [the `memchr` crate's `memmem`
routine][rust-memmem] and [libc's `memmem` routine][libc-memmem].

The example we use here was tested on Linux, although most of it should work on
macOS as well. Some parts of it can probably be adapted to work on Windows as
well.

[rust-memmem]: https://docs.rs/memchr/2.*/memchr/memmem/index.html
[libc-memmem]: https://man7.org/linux/man-pages/man3/memmem.3.html

## Initial setup

Let's start by creating a directory that will contain our benchmark definitions
and our program for collecting measurements. The directory can be anywhere:

```
$ mkdir -p byob/{benchmarks,runner}
$ mkdir byob/benchmarks/{definitions,haystacks}
$ cd byob
```

Now let's do a little setup for our runner program, which will be written
in Rust. We bring in a dependency on `memchr` (which provides a Rust
implementation of `memmem`) and `libc` (which provides FFI bindings to libc's
`memmem`):

```
$ echo 'fn main() {}' > runner/main.rs
$ cargo init ./runner/
$ cargo add --manifest-path runner/Cargo.toml memchr libc
```

And check that the stub runner program builds:

```
$ cargo build --release --manifest-path runner/Cargo.toml
```

Finally, let's grab a haystack to use for benchmarking. We'll use [_The
Adventures of Sherlock Holmes_ from Project Gutenberg][gutenberg-sherlock]:

```
$ curl -L 'https://www.gutenberg.org/files/1661/1661-0.txt' > benchmarks/haystacks/sherlock.txt
```

[gutenberg-sherlock]: https://www.gutenberg.org/files/1661/1661-0.txt

## First benchmark definition

Before starting on our runner program, it's useful to have at least one
benchmark definition to test with. So let's add one:

```toml
[[bench]]
model = "iter"
name = "sherlock-holmes"
regex = "Sherlock Holmes"
haystack = { path = "sherlock.txt" }
count = 91
engines = []
```

There are actually a few decisions that we're making by writing this defintion:

* There is a model named `iter`. It's up to us what it represents, but the
idea is that implementations of the model will look for all matches of the
needle in the haystack. We'll see this more concretely when we write our runner
program.
* `memmem` of course does not support regexes, so we only write a literal
string here even though the field is called `regex`.
* For now, we leave `engines` empty because `rebar` will complain if you add an
entry to the list that doesn't refer to an engine that it knows about. Since
we haven't defined any engines yet, we leave it empty. We'll define the engines
after we've written a runner program.

Now that we have a benchmark definition, we can ask rebar to convert it to the
[KLV](KLV.md) format:

```
$ rebar klv memmem/sherlock-holmes | head
name:22:memmem/sherlock-holmes
model:4:iter
case-insensitive:5:false
unicode:5:false
max-iters:1:0
max-warmup-iters:1:0
max-time:1:0
max-warmup-time:1:0
pattern:15:Sherlock Holmes
haystack:607430:﻿The Project Gutenberg eBook of The Adventures of Sherlock Holmes, by Arthur Conan Doyle
```

We'll eventually use the `rebar klv` command to test our runner program before
defining the engines. This tightens the feedback loop and makes it clearer what
the precise inputs to our runner program are and its behavior.

## The runner program

The runner program is what actually runs a `memmem` function repeatedly, and
collects a sample (consisting of the duration and count) for each execution.
These samples are then printed to `stdout` and read by `rebar`. The input to
the runner program is the KLV data shown in the previous section, corresponding
to the output of `rebar klv memmem/sherlock-holmes`.

(Note that adding a new runner program is also described in a little bit of
detail in [CONTRIBUTING](CONTRIBUTING.md#adding-a-new-regex-engine). Although
those instructions are somewhat specific to the regex barometer. In this guide,
we will build our own runner program for our own benchmark from soup to nuts.)

There are a few parts to rebar runner programs:

* Parsing a single item in the KLV format.
* Combining all of the items into a single "configuration" object. This
configuration object instructs the runner program what to do.
* The implementation of each model for each version of `memmem` that we want
to measure. (In this case, for simplicity, we combine everything into one
program, but you might want to split different implementations of `memmem` into
different programs. It really just depends on what works best.)
* Choosing and repeatedly executing the model according to the configuration.
* Gathering samples and printing them in a comma delimited format to `stdout`.

We'll go over each of these sections below. If you just want the source
code for the program without dissecting each part, then you can find it at
[`byob/runner/main.rs`](byob/runner/main.rs).

### Preamble

Open `runner/main.rs` in your favorite text editor. If you followed the
instructions above, it should just contain the following:

```rust
fn main() {}
```

You can leave that be for now. But add this above the `main` function:

```rust
use std::{
    io::Write,
    time::{Duration, Instant},
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

macro_rules! err {
    ($($tt:tt)*) => {
        return Err(From::from(format!($($tt)*)))
    }
}
```

This provides the imports we'll use. It also defines a convenient type
alias for representing fallible operations, along with an `err` macro for
conveniently creating and returning an error. You could also use `anyhow` for
error reporting, but for a simple runner program that isn't usually invoked
directly by end users, this is good enough.

### Parsing one KLV item

Let's write the code for parsing a single key-length-value (KLV) item from the
input given to us. First, define the type:

```rust
#[derive(Clone, Debug)]
struct OneKLV {
    key: String,
    value: String,
    len: usize,
}
```

The `len` field indicates the number of _bytes_ that the entire KLV item used.
This way, higher level code knows how many bytes to skip ahead to parse the
next KLV item. We also assume valid UTF-8 here, but if we wanted to benchmark
execution of `memmem` on invalid UTF-8, that'd be possible too. We just stick
to valid UTF-8 here in order to keep things a little simpler.

Parsing a single KLV item generally involves parsing three fields delimited
by `:`, with the final field always containing a trailing `\n` that isn't part
of the value. For the full details on the format, see [KLV](KLV.md).

The code to parse a single KLV item from the beginning of a `&str`:

```rust
impl OneKLV {
    fn read(mut raw: &str) -> Result<OneKLV> {
        let Some(key_end) = raw.find(':') else {
            err!("invalid KLV item: could not find first ':'")
        };
        let key = &raw[..key_end];
        raw = &raw[key_end + 1..];

        let Some(value_len_end) = raw.find(':') else {
            err!("invalid KLV item: could not find second ':' for '{key}'")
        };
        let value_len_str = &raw[..value_len_end];
        raw = &raw[value_len_end + 1..];

        let Ok(value_len) = value_len_str.parse() else {
            err!(
                "invalid KLV item: value length '{value_len_str}' \
                 is not a number for '{key}'",
            )
        };
        let value = &raw[..value_len];
        if raw.as_bytes()[value_len] != b'\n' {
            err!("invalid KLV item: no line terminator for '{key}'")
        }
        let len = key.len() + 1 + value_len_end + 1 + value.len() + 1;
        Ok(OneKLV { key: key.to_string(), value: value.to_string(), len })
    }
}
```

### Combining each KLV item into one configuration object

Now that we can parse one KLV item, we just need to parse all of them and
combine them into a single configuration object. This object will tell the
program which benchmark to run. We'll start with the `Config` type:

```rust
#[derive(Clone, Debug, Default)]
struct Config {
    name: String,
    model: String,
    needle: String,
    haystack: String,
    max_iters: u64,
    max_warmup_iters: u64,
    max_time: Duration,
    max_warmup_time: Duration,
}
```

We don't actually capture all possible KLV items because we don't need them
for benchmarking `memmem`. We ignore anything we don't need or recognize. For
example, the `case-insensitive` and `unicode` settings aren't relevant for
`memmem`.

Parsing all of the KLV items into a `Config` object is just a simple loop that
plucks one KLV item until the input has been exhausted:

```rust
impl Config {
    fn read(mut raw: &str) -> Result<Config> {
        let mut config = Config::default();
        while !raw.is_empty() {
            let klv = OneKLV::read(raw)?;
            raw = &raw[klv.len..];
            config.set(klv)?;
        }
        Ok(config)
    }

    fn set(&mut self, klv: OneKLV) -> Result<()> {
        let parse_duration = |v: String| -> Result<Duration> {
            Ok(Duration::from_nanos(v.parse()?))
        };
        let OneKLV { key, value, .. } = klv;
        match &*key {
            "name" => self.name = value,
            "model" => self.model = value,
            "pattern" => self.needle = value,
            "haystack" => self.haystack = value,
            "max-iters" => self.max_iters = value.parse()?,
            "max-warmup-iters" => self.max_warmup_iters = value.parse()?,
            "max-time" => self.max_time = parse_duration(value)?,
            "max-warmup-time" => self.max_warmup_time = parse_duration(value)?,
            _ => {}
        }
        Ok(())
    }
}
```

### A generic routine for benchmarking

Before getting to the actual implementation that calls a `memmem` routine
for benchmarking, we'll take a brief detour and write a generic routine that
repeatedly runs a function for a period of time (or up to a maximum number of
iterations), records a duration and result count sample for each function call,
and then returns all recorded samples.

First, let's define what a sample is:

```rust
#[derive(Clone, Debug)]
struct Sample {
    duration: Duration,
    count: usize,
}
```

And now the function, which also handles a "warm-up" phase where the function
is executed but no samples are gathered:

```rust
fn run(c: &Config, mut bench: impl FnMut() -> usize) -> Vec<Sample> {
    let warmup_start = Instant::now();
    for _ in 0..c.max_warmup_iters {
        let _count = bench();
        if warmup_start.elapsed() >= c.max_warmup_time {
            break;
        }
    }

    let mut samples = vec![];
    let run_start = Instant::now();
    for _ in 0..c.max_iters {
        let bench_start = Instant::now();
        let count = bench();
        let duration = bench_start.elapsed();
        samples.push(Sample { duration, count });
        if run_start.elapsed() >= c.max_time {
            break;
        }
    }
    samples
}
```

Basically, this accepts a `Config` that we parsed above and a closure that
executes arbitrary code and returns a count. In our case, this is the function
that will execute a single unit of work according to the model we're gathering
measurements for.

### Implementing the `iter` model

Finally, we can implement the actual calls to `memmem` that we want to measure.
First up is `memmem` from the Rust `memchr` crate:

```rust
fn rust_memmem_iter(c: &Config) -> Vec<Sample> {
    let finder = memchr::memmem::Finder::new(&c.needle);
    run(c, || {
        let mut haystack = c.haystack.as_bytes();
        let mut count = 0;
        while let Some(i) = finder.find(&haystack) {
            count += 1;
            haystack =
                match haystack.get(i + std::cmp::max(1, c.needle.len())..) {
                    Some(haystack) => haystack,
                    None => break,
                };
        }
        count
    })
}
```

Remember, our `iter` model is benchmarking the operation of "return a count of
all matches in the haystack." So each execution should be a complete iteration
of all matches. (Notice that we handle the case of an empty needle by ensuring
that `haystack` is always smaller after each iteration, which guarantees
termination.)

And now let's write the same implementation, but using libc's `memmem`. This
needs a little bit more code because we write a safe wrapper around `memmem`
and then use that in our benchmark instead.

```rust
fn libc_memmem_iter(c: &Config) -> Vec<Sample> {
    run(c, || {
        let mut haystack = c.haystack.as_bytes();
        let mut count = 0;
        while let Some(i) = libc_memmem(&haystack, c.needle.as_bytes()) {
            count += 1;
            haystack =
                match haystack.get(i + std::cmp::max(1, c.needle.len())..) {
                    Some(haystack) => haystack,
                    None => break,
                };
        }
        count
    })
}

/// A safe wrapper around libc's `memmem` function. In particular, this
/// converts memmem's pointer return to an index offset into `haystack`.
fn libc_memmem(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    // SAFETY: We know that both our haystack and needle pointers are valid and
    // non-null, and we also know that the lengths of each corresponds to the
    // number of bytes at that memory region.
    let p = unsafe {
        libc::memmem(
            haystack.as_ptr().cast(),
            haystack.len(),
            needle.as_ptr().cast(),
            needle.len(),
        )
    };
    if p.is_null() {
        None
    } else {
        let start = (p as isize) - (haystack.as_ptr() as isize);
        Some(start as usize)
    }
}
```

The wrapper is not precisely free, since it does have a null-pointer check and
a subtraction. Most correct uses of `memmem` are going to do at least a null
pointer check anyway. With that said, it is possible that this is not a correct
choice to make, but it depends on what you care about measuring. I leave it as
an exercise to the reader to make adjustments as you see fit.

### Putting it all together

Finally, we can put everything together. Here, we'll change our `main` function
to actually do something. Specifically, it should:

* Handle a query for the version of the runner program, which is recorded with
every measurement. (Along with the version of the rebar tool itself.)
* Provide a way to specify which engine is being executed. (The KLV data does
not include that, because every engine corresponds to its own program
execution. Since we use the same program to handle multiple engines, we need
some other channel of information to determine which engine we'll measure.)
* Read all of the KLV data from `stdin`.
* Parse the KLV data into a `Config` object.
* Based on the engine and config model given, select the implementation of
`memmem` to measure.
* Run it, collect the samples and print them to stdout.

Here's the code that does just that:

```rust
fn main() -> Result<()> {
    let Some(arg) = std::env::args_os().nth(1) else {
        err!("Usage: runner (<engine-name> | --version)")
    };
    let Ok(arg) = arg.into_string() else {
        err!("argument given is not valid UTF-8")
    };
    if arg == "--version" {
        writeln!(std::io::stdout(), env!("CARGO_PKG_VERSION"))?;
        return Ok(());
    }
    let engine = arg;
    let raw = std::io::read_to_string(std::io::stdin())?;
    let config = Config::read(&raw)?;
    let samples = match (&*engine, &*config.model) {
        ("rust/memmem", "iter") => rust_memmem_iter(&config),
        ("libc/memmem", "iter") => libc_memmem_iter(&config),
        (engine, model) => {
            err!("unrecognized engine '{engine}' and model '{model}'")
        }
    };
    let mut stdout = std::io::stdout().lock();
    for s in samples.iter() {
        writeln!(stdout, "{},{}", s.duration.as_nanos(), s.count)?;
    }
    Ok(())
}
```

### Testing the runner program

Before moving on to the next step, it would be a good idea to test the runner
program. This is what you might normally do if you were writing your own runner
program.

First, let's make sure our runner program has been built:

```
$ cargo build --release --manifest-path runner/Cargo.toml
```

And now, as we did above, we can use the `rebar klv` command to convert our
benchmark definition to structured data and then pass that into our runner
program via `stdin`. We also choose to run the `rust/memmem` engine by passing
it as the first command line argument to the runner program.

```
$ rebar klv memmem/sherlock-holmes | ./runner/target/release/runner rust/memmem
$
```

Hmmm, what went wrong here? Shouldn't some samples be printed? The reason why
nothing was printed is because we didn't tell `rebar klv` how much time to
spend benchmarking, nor did we tell it how many iterations to use. By default,
both of them are zero. So let's give it some non-zero values:

```
$ rebar klv memmem/sherlock-holmes --max-iters 10 --max-time 3s | ./runner/target/release/runner rust/memmem
91414,91
85247,91
85014,91
85137,91
84886,91
84579,91
84604,91
84665,91
84376,91
84468,91
```

We can also test our `libc/memmem` engine:

```
$ rebar klv memmem/sherlock-holmes --max-iters 10 --max-time 3s | ./runner/target/release/runner libc/memmem
298582,91
285372,91
276867,91
272436,91
286887,91
266991,91
265299,91
264227,91
263805,91
263200,91
```

## Defining the engines

In order to use rebar to build the runner program and gather measurements,
we have to actually teach rebar how to do it. We have tell it how to get the
version, how to build it, how to run it and how to clean any build artifacts.

The engines are defined in `benchmarks/engines.toml`, which we created above
but left it empty. Here's the `rust/memmem` engine:

```toml
[[engine]]
  name = "rust/memmem"
  cwd = "../runner"
  [engine.version]
    bin = "./target/release/runner"
    args = ["--version"]
  [engine.run]
    bin = "./target/release/runner"
    args = ["rust/memmem"]
  [[engine.build]]
    bin = "cargo"
    args = ["build", "--release"]
  [[engine.clean]]
    bin = "cargo"
    args = ["clean"]
```

And the `libc/memmem` engine is very similar. The only difference is the
first argument we pass when running the program:

```toml
[[engine]]
  name = "libc/memmem"
  cwd = "../runner"
  [engine.version]
    bin = "./target/release/runner"
    args = ["--version"]
  [engine.run]
    bin = "./target/release/runner"
    args = ["libc/memmem"]
  [[engine.build]]
    bin = "cargo"
    args = ["build", "--release"]
  [[engine.clean]]
    bin = "cargo"
    args = ["clean"]
```

## Collecting measurements

Once you've defined the engines, you should be able to build them:

```
$ rebar build
rust/memmem: running: cd "runner" && "cargo" "build" "--release"
rust/memmem: build complete for version 0.1.0
libc/memmem: running: cd "runner" && "cargo" "build" "--release"
libc/memmem: build complete for version 0.1.0
```

And then test that measurements can be gathered and the result is what is
expected:

```
$ rebar measure -t
memmem/sherlock-holmes,iter,libc/memmem,0.1.0,OK
memmem/sherlock-holmes,iter,rust/memmem,0.1.0,OK
```

Now we can gather measurements. We use `tee` here so that you can see the
progress of recording measurements (since it can take a while, especially when
the number of measurements grows), and also so that the measurements are saved
somewhere for later analysis.

```
$ rebar measure | tee results.csv
name,model,rebar_version,engine,engine_version,err,haystack_len,iters,total,median,mad,mean,stddev,min,max
memmem/sherlock-holmes,iter,0.0.1 (rev e5d648ff17),libc/memmem,0.1.0,,607430,50577,4.57s,59.28us,0.00ns,59.28us,3.16us,53.54us,70.32us
memmem/sherlock-holmes,iter,0.0.1 (rev e5d648ff17),rust/memmem,0.1.0,,607430,209468,4.62s,14.49us,0.00ns,14.29us,343.00ns,13.51us,26.04us
```

Finally, we can compare the results in a more palatable form:

```
$ rebar cmp results.csv
benchmark               libc/memmem       rust/memmem
---------               -----------       -----------
memmem/sherlock-holmes  9.5 GB/s (4.09x)  39.0 GB/s (1.00x)
```

## Bonus: measuring musl's memmem implementation

If you've followed along so far and are on Linux, it's very likely that the
runner program you've written is measuring [glibc's][glibc]'s `memmem` routine.
This is because Rust programs on Linux will by default dynamically link with
your system's libc, and most Linux systems use glibc by default. (But not all.)

(If you're on a different platform like macOS, then your program is likely
measuring whatever implementation of `memmem` is provided by your platform.
This is why the engine is called `libc/memmem` and not, for example,
`glibc/memmem`. Because there's nothing about our setup here that specifically
chooses a particular libc implementation.)

But what if you did want to measure a particular libc implementation? Depending
on the environment, this could be quite tricky for a variety of reasons. But
if you're on Linux x86_64, have [rustup] and [musl] installed, then it's
very easy to add a new engine to our harness that measures musl's `memmem`
implementation. First, make the musl target available to Cargo:

[rustup]: https://rustup.rs/
[musl]: https://www.musl-libc.org/
[glibc]: https://www.gnu.org/software/libc/

```
$ rustup target add x86_64-unknown-linux-musl
```

Then add the following engine definition to `benchmarks/engines.toml`:

```toml
[[engine]]
  name = "musl/memmem"
  cwd = "../runner"
  [engine.version]
    bin = "./target/x86_64-unknown-linux-musl/release/runner"
    args = ["--version"]
  [engine.run]
    bin = "./target/x86_64-unknown-linux-musl/release/runner"
    args = ["libc/memmem"]
  [[engine.build]]
    bin = "cargo"
    args = ["build", "--release", "--target", "x86_64-unknown-linux-musl"]
  [[engine.clean]]
    bin = "cargo"
    args = ["clean", "--target", "x86_64-unknown-linux-musl"]
```

Notice that this is the same as the existing `libc/memmem` definition, except
we add a `--target x86_64-unknown-linux-musl` argument to the build command and
subsequently tweak the path to the runner binary itself.

Now add `musl/memmem` to the list of engines to get measurements for in our
benchmarl, so that the list in `benchmarks/definitions/memmem.toml` now looks
like this:

```toml
engines = [
  "libc/memmem",
  "musl/memmem",
  "rust/memmem",
]
```

And that's it. We don't even need to change the program since we re-use the
program's `libc/memmem` engine. Remember, it doesn't care about _which_ libc
is used. We control that through the linking step in the build process (which
is mostly hidden from us, but is a consequence of us using the musl target).

Now just rebuild the runner programs. You should now see the `musl/memmem`
engine:

```
$ rebar build
rust/memmem: running: cd "runner" && "cargo" "build" "--release"
rust/memmem: build complete for version 0.1.0
libc/memmem: running: cd "runner" && "cargo" "build" "--release"
libc/memmem: build complete for version 0.1.0
musl/memmem: running: cd "runner" && "cargo" "build" "--release" "--target" "x86_64-unknown-linux-musl"
musl/memmem: build complete for version 0.1.0
```

Test that things work. In particular, ensure that `musl/memmem` shows up
here. If you forgot to add `musl/memmem` to your list of engines for the
`sherlock-holmes` benchmark above, then it won't show up here.

```
$ rebar measure -t
memmem/sherlock-holmes,iter,libc/memmem,0.1.0,OK
memmem/sherlock-holmes,iter,musl/memmem,0.1.0,OK
memmem/sherlock-holmes,iter,rust/memmem,0.1.0,OK
```

Next we can gather measurements:

```
$ rebar measure | tee results.csv
name,model,rebar_version,engine,engine_version,err,haystack_len,iters,total,median,mad,mean,stddev,min,max
memmem/sherlock-holmes,iter,0.0.1 (rev e5d648ff17),libc/memmem,0.1.0,,607430,53312,4.57s,54.68us,0.00ns,56.24us,3.00us,53.07us,75.35us
memmem/sherlock-holmes,iter,0.0.1 (rev e5d648ff17),musl/memmem,0.1.0,,607430,13508,4.51s,221.79us,0.00ns,222.05us,1.57us,219.25us,246.42us
memmem/sherlock-holmes,iter,0.0.1 (rev e5d648ff17),rust/memmem,0.1.0,,607430,215489,4.62s,13.87us,0.00ns,13.89us,108.00ns,13.59us,18.44us
```

And finally, compare them in a palatable way:

```
$ rebar cmp results.csv
benchmark               libc/memmem        musl/memmem        rust/memmem
---------               -----------        -----------        -----------
memmem/sherlock-holmes  10.3 GB/s (3.94x)  2.6 GB/s (15.99x)  40.8 GB/s (1.00x)
```

## Bonus: stooping to libc's level

If you were paying really close attention while we built our runner program,
you might have noticed that for the libc `memmem` implementation, we call
`memmem` for every search:

```rust
while let Some(i) = libc_memmem(&haystack, c.needle.as_bytes()) {
    count += 1;
    haystack =
        match haystack.get(i + std::cmp::max(1, c.needle.len())..) {
            Some(haystack) => haystack,
            None => break,
        };
}
```

Where as for the `memchr` crate's implementation of `memmem`, we call a
function that only accepts the haystack, not the needle:

```rust
while let Some(i) = finder.find(&haystack) {
    count += 1;
    haystack =
        match haystack.get(i + std::cmp::max(1, c.needle.len())..) {
            Some(haystack) => haystack,
            None => break,
        };
}
```

What gives? Well, it turns out that libc's `memmem` API nearly requires it to
rebuild its internal searcher on every call. That is, there is no API that
let's you say, "build a searcher with this needle and then use that built
searcher to look for occurrences in many different haystacks." This turns out
to be a _really_ common use case. For example, iterating over the lines in a
file and looking for lines containing a certain substring. `memmem` has to
repeatedly rebuild its searcher every single freaking time. (This is probably
why there is no end to articles about how "my naive substring search algorithm
beats `libc`'s hyper-optimized `memmem` routine! See, the trick to optimization
really is to just write simple code!")

In contrast, the `memchr` crate provides an API for building a
[`Finder`][memchr-memmem-finder] once, and then using it to execute many
searches. This is a legitimate advantage to better API design, and is in my
opinion fair game. Still though, what if you were thinking about implementing
your own libc? Legacy requires that you provide a `memmem` API. So how well
would the `memchr` crate fair?

To figure this out, we should do two things:

* Define a new benchmark where this API difference probably matters. The one
benchmark we've already defined only has 91 matches, which _probably_ is small
enough that this API difference doesn't lead to a legitimate improvement. (The
API difference is a latency optimization, not a throughput one. Although, the
more restrictive API might prevent one from doing more throughput optimizations
if they would too negatively impact latency!)
* Define a new `rust/memmem/restricted` engine that always rebuilds the
searcher for every search. We _could_ just change the code of `rust/memmem`,
rebuild the runner program and then recapture measurements. And sometimes that
might be the right thing to do. But in this case, it would be nice to have both
options available simultaneously since this really comes down to a public API
difference and not some internal tweak that we're testing.

First, let's define a new benchmark. We'll look for all occurrences of `he`,
which is about two orders of magnitude more common than `Sherlock Holmes`:

```toml
[[bench]]
model = "iter"
name = "very-common"
regex = "he"
haystack = { path = "sherlock.txt" }
count = 11_706
engines = [
  "libc/memmem",
  "musl/memmem",
  "rust/memmem",
]
```

Now teach the new engine to rebar by adding it to `benchmarks/engines.toml`:

```toml
[[engine]]
  name = "rust/memmem/restricted"
  cwd = "../runner"
  [engine.version]
    bin = "./target/release/runner"
    args = ["--version"]
  [engine.run]
    bin = "./target/release/runner"
    args = ["rust/memmem/restricted"]
  [[engine.build]]
    bin = "cargo"
    args = ["build", "--release"]
  [[engine.clean]]
    bin = "cargo"
    args = ["clean"]
```

And add `rust/memmem/restricted` to both of our benchmark definitions, so that
the list in _both_ looks like this:

```toml
engines = [
  "libc/memmem",
  "musl/memmem",
  "rust/memmem",
  "rust/memmem/restricted",
]
```

As usual, test that everything works:

```
$ rebar measure -t
memmem/sherlock-holmes,iter,libc/memmem,0.1.0,OK
memmem/sherlock-holmes,iter,musl/memmem,0.1.0,OK
memmem/sherlock-holmes,iter,rust/memmem,0.1.0,OK
memmem/sherlock-holmes,iter,rust/memmem/restricted,0.1.0,OK
memmem/very-common,iter,libc/memmem,0.1.0,OK
memmem/very-common,iter,musl/memmem,0.1.0,OK
memmem/very-common,iter,rust/memmem,0.1.0,OK
memmem/very-common,iter,rust/memmem/restricted,0.1.0,OK
```

Then collect measurements:

```
$ rebar measure | tee results.csv
name,model,rebar_version,engine,engine_version,err,haystack_len,iters,total,median,mad,mean,stddev,min,max
memmem/sherlock-holmes,iter,0.0.1 (rev e5d648ff17),libc/memmem,0.1.0,,607430,51058,4.57s,58.94us,0.00ns,58.72us,3.48us,53.08us,86.91us
memmem/sherlock-holmes,iter,0.0.1 (rev e5d648ff17),musl/memmem,0.1.0,,607430,12463,4.51s,243.65us,0.00ns,240.67us,4.32us,233.96us,255.69us
memmem/sherlock-holmes,iter,0.0.1 (rev e5d648ff17),rust/memmem,0.1.0,,607430,209354,4.62s,14.06us,0.00ns,14.30us,357.00ns,13.54us,21.91us
memmem/sherlock-holmes,iter,0.0.1 (rev e5d648ff17),rust/memmem/restricted,0.1.0,,607430,186450,4.62s,15.98us,0.00ns,16.06us,212.00ns,15.72us,22.69us
memmem/very-common,iter,0.0.1 (rev e5d648ff17),libc/memmem,0.1.0,,607430,8601,4.51s,345.96us,0.00ns,348.78us,6.36us,339.93us,394.50us
memmem/very-common,iter,0.0.1 (rev e5d648ff17),musl/memmem,0.1.0,,607430,6966,4.51s,429.51us,0.00ns,430.68us,5.09us,426.53us,479.23us
memmem/very-common,iter,0.0.1 (rev e5d648ff17),rust/memmem,0.1.0,,607430,19255,4.51s,154.69us,0.00ns,155.77us,4.33us,152.76us,183.47us
memmem/very-common,iter,0.0.1 (rev e5d648ff17),rust/memmem/restricted,0.1.0,,607430,11390,4.51s,263.38us,0.00ns,263.36us,2.79us,256.52us,303.20us
```

And compare the results:

```
$ rebar cmp results.csv
benchmark               libc/memmem          musl/memmem          rust/memmem        rust/memmem/restricted
---------               -----------          -----------          -----------        ----------------------
memmem/sherlock-holmes  9.6 GB/s (4.19x)     2.3 GB/s (17.33x)    40.2 GB/s (1.00x)  35.4 GB/s (1.14x)
memmem/very-common      1674.4 MB/s (2.24x)  1348.7 MB/s (2.78x)  3.7 GB/s (1.00x)   2.1 GB/s (1.70x)
```

Firstly, it looks like there is a small but measurable difference in total
throughput for the `Sherlock Holmes` search. That is, rebuilding the searcher
90 times is enough to ding the throughput a little bit.

Secondly, the difference between `rust/memmem` and `rust/memmem/restricted` is
much higher for the `very-common` benchmark. In this case, the searcher is
rebuilt much more, and as a result, the overall throughput is about 2x slower.

We'll stop our investigation there. The point here isn't necessarily to do a
full analysis on `memmem` performance, but to show how one might go about the
_process_ of building their own benchmark suite. And in particular, it's
important to show the flexibility of the engine and model concepts built into
rebar. When taken together, they can lead to capturing a variety of different
types of workloads and configurations.

[memchr-memmem-finder]: https://docs.rs/memchr/latest/memchr/memmem/struct.FindIter.html
