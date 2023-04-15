This document describes how to build the `rebar` tool and the regex engines
that it knows how to measure. For a guided tour of how to *use* `rebar`,
see the [TUTORIAL](TUTORIAL.md).

## Platform support

I've only tested `rebar` on Linux. It should probably work on macOS too.
`rebar` itself will probably work on Windows as well, but building at least
some of the regex engines may be a challenge. As will be discussed below, you
don't need to build all of the regex engines in order to use `rebar`. It works
perfectly fine if a few cannot be built. Moreover, even if you can't build any
of them, you can still use `rebar` to explore the benchmark data. (See the
tutorial linked above for a guided exploration.)

## Building `rebar`

`rebar` is the harness tool used to gather measurements and compare results.
The harness tool itself doesn't have any explicit knowledge about regex engines
or even benchmarks. It just knows how to read a directory full of TOML files,
and those TOML files define the benchmarks and tell `rebar` how to run a
particular regex engine.

Unless you're [building your own benchmark suite](BYOB.md), it's likely that
`rebar` will not be useful as a tool on its own. Instead, you'll want the
benchmark definitions and regex engine runner programs. You can get all of that
by simply cloning this repository:

```
$ git clone https://github.com/BurntSushi/rebar
$ cd rebar
```

Building the harness tool requires that Rust be installed. If you're on Linux,
you can probably install Rust via your distribution's package manager, but it
might give you a version of Rust that is too old to build `rebar`. If so, you
might consider [installing Rust via `rustup`](https://rustup.rs).

Once you have Rust installed, you should have a command called `cargo`, which
we use to build `rebar`:

```
$ cargo install --path .
```

On my system, this compiles the `rebar` binary and puts it at
`$HOME/.local/cargo/bin/rebar`, which is already in my `PATH`. If you can't
find it, the binary will also be at `./target/release/rebar`. You can test that
`rebar` is available by querying its version (the output you see may be a
little different than what is shown below):

```
$ rebar version
0.0.1 (rev b46c3a4aba)
```

## Building the regex engines

The runner programs that execute regex searches can be built by `rebar` with
the following command:

```
$ rebar build
```

It is almost certain that some regex engines will fail to build initially,
and this is usually because of a missing dependency. Below, we'll attempt to
explain how `rebar` builds runner programs so that you can diagnose the issue
and install the missing dependency. Do note though that `rebar` is perfectly
happy to run without building all of the regex engines. So you don't have to
get `rebar build` working completely.

### How `rebar` knows about each regex engine

All regex engines that `rebar` knows how to run are defined in
[`benchmarks/engines.toml`](benchmarks/engines.toml). Each entry in that file
provides the following details about each engine:

* Its name.
* The program dependencies the engine has.
* How to build the engine.
* How to get the version of the engine after it has been built.
* How to run the engine.
* How to remove the build artifacts.

`rebar` does not provide a way to build every regex engine it benchmarks from
source. *Some* are done this way:

* The PCRE2 and RE2 regex engines have their source code vendored into this
repository. That means you don't need to have them separately installed.
`rebar` knows how to build them from scratch. All you need are a C and C++
compiler, respectively.
* All regex engines written in Rust are pinned to a specific version.

Most of the rest of the regex engines are instead pulled in through system
dependencies. For example, the `javascript/v8` regex engine is benchmarked via
a Javascript program that is executed by your system's copy of [Node]. `rebar`
will not install or build Node for you. Instead, if Node is not installed,
building the `javascript/v8` engine will simply fail. To see what each engine
requires, inspect its definition in `benchmarks/engines.toml`.

[Node]: https://nodejs.org

With that said, most regex engines, even if they are pulled in from a system
dependencies, do require an actual program to be compiled. For example, the
`dotnet` regex engine is bundled as part of your .NET system installation,
but this repository contains a .NET program that uses that bundled regex
engine. The program itself still needs to be compiled.

As a result, there are generally three classes of regex engine in `rebar`:

* Regex engines that just need a system dependency to be installed and
otherwise do not require any building. For example, `javascript/v8`, `perl`
and `python/re`. These engines are still "built," but the process of building
is just verifying that the programs can be executed.
* Regex engines that derive their regex engine from a system dependency, but
still need a program compiled before it can be executed. For example, `dotnet`,
`java/hotspot`, `icu` and `go/regexp`.
* Regex engines that are either bundled or pinned such that they themselves
are built by `rebar`. For example, `rust/regex`, `re2`, `pcre2` and `regress`.
Note that these still need system dependencies. For example, a Rust compiler,
a C++ compiler and a C compiler.

There isn't really any significance to which class a regex engine falls into.
It's just what is most convenient. For example, it is feasible to build PCRE2
from scratch, but much more annoying to build all of Python just to use its
standard library `re` module.

The downside of this convenience-based approach is that `rebar` cannot
benchmark arbitrary versions of regex engines. That is, in many cases, you are
pretty much stuck with whatever version happens to be installed on your system.
This might make benchmark results incomparable from different environments.
Currently, `rebar` doesn't have a great answer to this.

### Getting more information from `rebar build`

This section will give a brief lesson on how to use `rebar build` by showing
a debugging session. For example, let's say I don't have .NET installed on
my system, but I want to benchmark its regex engine. I don't care about the
others, so I ask `rebar` to just build the .NET runner programs:

```
$ rebar build -e dotnet
dotnet: dependency command failed: failed to run command and wait for output
note: a dependency that is required to build 'dotnet' could not be found, either because it isn't installed or because it didn't behave as expected
note: run `RUST_LOG=debug rebar build -e '^dotnet$'` to see more details
dotnet/compiled: dependency command failed: failed to run command and wait for output
dotnet/nobacktrack: dependency command failed: failed to run command and wait for output
```

The error messages above are telling you that `rebar` can't find a program
dependency. Notice also that there are multiple regex engines that contain
the string `dotnet`. This is because `dotnet` has three different regex
engines that might be worth measuring: the default interpreter, a JIT and a
non-backtracking finite automata based engine. Generally speaking, since all
three are executed by the same runner program, if we can build one of them then
we'll be able to build all of them. So let's just focus on one. We'll also set
`RUST_LOG=debug` so that we can get a little more information about what's
failing:

```
$ RUST_LOG=debug rebar build -e '^dotnet$'
[2023-04-15T18:20:18Z DEBUG rebar::util] running command: cd "engines/dotnet" && "/home/andrew/code/rust/rebar/engines/dotnet/bin/Release/net7.0/main" "version"
[2023-04-15T18:20:18Z DEBUG rebar::format::benchmarks] extracted version for engine 'dotnet' failed: failed to get version: failed to run command and wait for output: No such file or directory (os error 2)
[2023-04-15T18:20:18Z DEBUG rebar::util] running command: "dotnet" "--list-sdks"
dotnet: dependency command failed: failed to run command and wait for output
note: a dependency that is required to build 'dotnet' could not be found, either because it isn't installed or because it didn't behave as expected
note: run `RUST_LOG=debug rebar build -e '^dotnet$'` to see more details
```

So why isn't `dotnet --list-sdks` working? Let's try it:

```
$ dotnet --list-sdks
zsh: command not found: dotnet
```

Ah, because .NET is not installed! So let's get it installed. You'll need to
follow [.NET installation instructions for your platform][dotnet-install], but
for me on Archlinux, I'll try this:

[dotnet-install]: https://dotnet.microsoft.com/en-us/download

```
$ sudo pacman -S dotnet-host
```

Now let's try building again:

```
$ rebar build -e '^dotnet$'
dotnet: dependency command did not print expected output: could not find match for "(?m)^7\\." in output of "dotnet" "--list-sdks"
note: a dependency that is required to build 'dotnet' could not be found, either because it isn't installed or because it didn't behave as expected
note: run `RUST_LOG=debug rebar build -e '^dotnet$'` to see more details
```

That didn't work, so let's do what the error message suggests and re-run it
with some debugging log messages displayed:

```
$ RUST_LOG=debug rebar build -e '^dotnet$'
[2023-04-11T14:27:18Z DEBUG rebar::util] running command: cd "engines/dotnet" && "/home/andrew/code/rust/rebar/engines/dotnet/bin/Release/net7.0/main" "version"
[2023-04-11T14:27:18Z DEBUG rebar::format::benchmarks] extracted version for engine 'dotnet' failed: failed to get version: failed to run command and wait for output: No such file or directory (os error 2)
[2023-04-11T14:27:18Z DEBUG rebar::util] running command: "dotnet" "--list-sdks"
dotnet: dependency command did not print expected output: could not find match for "(?m)^7\\." in output of "dotnet" "--list-sdks"
note: a dependency that is required to build 'dotnet' could not be found, either because it isn't installed or because it didn't behave as expected
note: run `RUST_LOG=debug rebar build -e '^dotnet$'` to see more details
[2023-04-11T14:27:18Z DEBUG rebar::cmd::build] output for dependency command "dotnet" "--list-sdks": <EMPTY>
```

The extra logging shows us `rebar` is trying to run `dotnet --list-sdks`, but
it's not getting the output it expects. Indeed, it claims that it gets no
output at all. Is that true?

```
$ dotnet --list-sdks
$
```

Yup. So we probably need to install more stuff. In this case, an SDK:

```
$ sudo pacman -S dotnet-sdk
```

And now let's try building again:

```
$ rebar build -e '^dotnet$'
dotnet: running: cd "engines/dotnet" && "dotnet" "build" "-c" "Release"
dotnet: build complete for version 7.0.3
```

Great! Now let's make sure it actually runs by executing `rebar`'s test suite
(masquerading as benchmarks). The `-t` flag below tells `rebar` to execute
the benchmark once and emit a success or fail message for each one. Since
`rebar` verifies the result of each benchmark, the benchmark definition can
double as a test by just running it once.

```
$ rebar measure -f '^test/' -e '^dotnet$' -t
test/func/leftmost-first,dotnet,count-spans,7.0.3,OK
test/func/dollar-only-matches-end,dotnet,count,7.0.3,OK
test/func/non-greedy,dotnet,count,7.0.3,OK
test/model/count,dotnet,count,7.0.3,OK
[... snip ...]
```

All is well! Finally, let's manufacture a build error with the .NET runner
program to see what that looks like. Let's change the first line of
[`engines/dotnet/Main.cs`](engines/dotnet/Main.cs) to this:

```
using Tystem;
```

Now let's see what happens when we try to build the runner program:

```
$ rebar build -e '^dotnet$'
dotnet: running: cd "engines/dotnet" && "dotnet" "build" "-c" "Release"
dotnet: build failed: command failed with ExitStatus(unix_wait_status(256)) but stderr is empty
note: run `RUST_LOG=debug rebar build -e '^dotnet$'` to see more details
```

That's not so helpful, so let's ask for debug messages:

```
$ RUST_LOG=debug rebar build -e '^dotnet$'
[2023-04-11T14:33:34Z DEBUG rebar::util] running command: cd "engines/dotnet" && "/home/andrew/code/rust/rebar/engines/dotnet/bin/Release/net7.0/main" "version"
[2023-04-11T14:33:34Z DEBUG rebar::util] running command: "dotnet" "--list-sdks"
dotnet: running: cd "engines/dotnet" && "dotnet" "build" "-c" "Release"
[2023-04-11T14:33:34Z DEBUG rebar::util] running command: cd "engines/dotnet" && "dotnet" "build" "-c" "Release"
[2023-04-11T14:33:35Z DEBUG rebar::util] command failed, exit status: ExitStatus(unix_wait_status(256))
[2023-04-11T14:33:35Z DEBUG rebar::util] stderr:
dotnet: build failed: command failed with ExitStatus(unix_wait_status(256)) but stderr is empty
note: run `RUST_LOG=debug rebar build -e '^dotnet$'` to see more details
```

That's not terribly helpful either. In particular, `stderr` appears to be empty
(otherwise it would have been included here). So maybe the problems are being
printed to `stdout` instead. Let's try to run the command that `rebar` is
running ourselves:

```
$ cd engines/dotnet
$ dotnet build -c Release
MSBuild version 17.4.1+fedecea9d for .NET
  Determining projects to restore...
  All projects are up-to-date for restore.
/home/andrew/code/rust/rebar/engines/dotnet/Main.cs(1,7): error CS0246: The type or namespace name 'Tystem' could not be found (are you missing a using directive or an assembly reference?) [/home/andrew/code/rust/rebar/engines/dotnet/main.csproj]

Build FAILED.

/home/andrew/code/rust/rebar/engines/dotnet/Main.cs(1,7): error CS0246: The type or namespace name 'Tystem' could not be found (are you missing a using directive or an assembly reference?) [/home/andrew/code/rust/rebar/engines/dotnet/main.csproj]
    0 Warning(s)
    1 Error(s)

Time Elapsed 00:00:01.04
```

There we go. Now we can undo our manufactured error and rebuild it:

```
$ cd ../..
$ rebar build -e '^dotnet$'
dotnet: running: cd "engines/dotnet" && "dotnet" "build" "-c" "Release"
dotnet: build complete for version 7.0.3
```
