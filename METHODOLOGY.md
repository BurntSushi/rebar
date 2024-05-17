This document describes the methodology of this regex barometer. In particular,
we go over the motivation for the problem this barometer tries to solve and its
high level design.

For details on the specific tasks we measure, see the [MODELS](MODELS.md)
document. This document instead tries to describe things at a higher level,
and in particular how the rebar harness works.

## The Problem

There were two primary reasons for why I developed this barometer:

1. I wanted a reliable way of tracking the performance of the Rust regex crate
on various workloads. In particular, I wanted it to be easy to add new
benchmarks, and I wanted the benchmark metholody to be capable of capturing a
wide array of use cases. In effect, I wanted to be able to translate an issue
filed by an end user of a performance problem into something that can be more
precisely measured and tracked over time. Ultimately, the benchmarks are
intended to facilitate profiling specific tasks and improving performance. The
number of benchmarks are meant to make coverage as comprehensive as possible,
since it's not uncommon for one benchmark to get slower after speeding up
another.
2. I wanted to understand where Rust's regex crate stands in terms of
performance relative to other regex engines. There are several extant regex
benchmarks that purport to do this, but they are all very incomplete across
multiple dimensions. (Both in the types of regexes they measure and also in the
types of workloads.) For example, I don't know of any public regex benchmark
that compares the performance of extracting the positions of capturing groups.

These are the two problems that I started with, and ultimately reflect the
motivation that drove the design.

We'll tease apart each half of the problem in the following two sections by
talking about the requirements I wanted to impose based on each half.

### Rust Regex Crate Performance

My estimation is that tracking performance of the regex crate benefits from the
following things:

* Flexibility for measuring varying workloads. So for example, we shouldn't be
limited to just counting the number of times a regex matches. We should also
measure the time it takes to find the positions matched by each capture group.
I also wanted to include a workload that iterates over the lines of some data
and runs a regex on each line, or in other words, a simple grep-like program.
* It should be very easy to add new benchmarks. Benchmarks don't just come from
the regexes that I think are interesting, they also come from looking at how
regexes are used in real world programs and also from user reports of slow
search times.
* Benchmarks should also be tests. If we add a new benchmark to measure over
time, we should also ensure that the result of the benchmark doesn't change.
* We should try to mitigate the effects of noise on measurements as much as
possible. Noise can make interpreting results over time tricky, since noise
can make it look like a measurement is an improvement or a regression, even
though it might just reflect natural variation in the measurement process.
* The benchmarking harness should support profiling. That is, it should be easy
to take a command that collects measurements, and transform it into a command
that runs a specific benchmark and is amenable to profiling. (i.e., Doesn't
spend too much time collecting measurements relative to the actual work being
measured.)

### Measuring Other Regex Engines

Measuring other regex engines presents its own unique challenges, but I believe
the following to be useful properties:

* Measurements should reflect "real world" use cases as much as possible. In
particular, this means not adding extra overhead just for the convenience of
benchmarking.
* The work required to add a new regex engine to the benchmark harness should
be small and simple. When given the choice between writing more code and
complex code, we should prefer more code over complex code.
* The benchmarks themselves should be defined in a common format that works
across all supported regex engines.
* Comparisons between engines should generally be as "apples to apples" as
possible. This means that gathering measurements should try to both model
real world workloads while also trying to keep the actual work being performed
as similar as possible.

## Design

This section of the document discusses the design of `rebar` by breaking it
down into three big components:

* `rebar` executes sub-processes, and those sub-processes are responsible for
actually compiling regexes and measuring their search times.
* The format used to communicate benchmark parameters to sub-processes.
* The format used to define benchmarks, which are read by `rebar`.

### Process Oriented Architecture

When I first started rebar (before it was even called rebar), I had scoped the
project to just the regex crate. That is, I only cared about the first half of
the problem described above. But then I started getting curious about how other
regex engines performed on similar tasks. And especially, I wanted to learn
about optimization techniques that other regex engines used that I might be
able to learn from.

So I started adding other regex engines to the predecessor of rebar. I started
with only the PCRE2 and RE2 regex engines, which I accomplished by binding them
through a C API and capturing measurements from Rust code. At the time, I told
myself that I would just limit myself to regex engines that could be reasonably
called through a C API without cost.

But I really wanted to compare measurements with Go and Python programs.
Namely, in the past, folks have reported roughly equivalent programs in Rust
and Python (and also Go), where both programs spent the majority of their time
in regex searching, but where the Python (and Go) programs were faster than the
corresponding Rust program. In order to capture these kinds of measurements, my
benchmark harness needed to evolve because neither Go's nor Python's regex
engines can be easily called through a C API.

I thought long and hard about whether it was worth taking this extra step,
because it would add a lot of complexity compared to where I started: just
benchmarking the Rust regex crate. I decided that I would have a lot of fun
building out a regex benchmark that I could publish, and that could potentially
supplant all existing public regex benchmarks (that I knew about).

So, the question then became, how do I balance the following things?

* Actually capturing measurements.
* Doing it without any added overhead.
* Making it relatively simple to add new regex engines.

I ended up settling on the following design points:

* rebar does not know about any specific regex engine that it measures. That
is, rebar doesn't bundle regex engines like RE2 or PCRE2. Instead, all
measurements are captured over a process boundary.
* The programs that execute a benchmark have to be responsible for timing those
measurements. I at first resisted this, because I wanted this sort of thing to
be under the control of a single piece of code that remains the same across
all measurements. But this is somewhat tricky to do reliably. We can't rely
on the time it takes to execute a process since many times are measured in
nano or micro seconds. The overhead of process creation would make capturing
such timings infeasible. Thankfully, most programming environments these days
provide a way to capture high resolution timings which makes it feasible for
programs to capture their own timings.
* The programs need to report a count from each benchmark execution back to
rebar, and rebar should verify that this count is what is expected. This is
critical for ensuring that the workload is what you expect it to be.
* rebar should otherwise be responsible for the aggregation of timings and
other analyses. That is, the runner programs just needs to run some code
repeatedly and collect a sample for each execution. The sample consists of the
time it took to execute and the count returned by it. Then, once "enough"
samples have been collected, they should be sent back to rebar.

Runner programs accept a simple format describing the benchmark (discussed in
the next section) on `stdin`, and must output samples in a comma-delimited
format to `stdout`. Each comma-delimited record contains the two fields
described above: a duration (in nanoseconds) and a verification count.

### Communicating Benchmark Parameters to Subprocesses

In order for a runner program to execute a benchmark, it needs to know a number
of things:

* The regex to compile.
* The haystack to search.
* Various flags, if any, that should be set. For example, whether to search
case insensitively or whether to enable Unicode mode.
* The type of workload to execute. (For example, finding a count of all matches
or finding a count of all matching capture groups.)
* Other parameters that control the measurement process, such as the maximum
number of iterations and the maximum time to wait.

The question, then, is how to provide these parameters to a runner program.
There are a number of different approaches:

1. Teach each runner program how to read the TOML benchmark definitions. (But
note that the TOML definitions don't include parameters like "max number of
iterations," which are instead defined at measurement time by the user of
`rebar`. So we'd still need some other channel---probably CLI flags---to pass
those parameters.)
2. Pass the haystack on `stdin` (since it can be large) and pass all other
parameters as command line flags.
3. Serialize the benchmark definition to some format, and pass it over stdin
or via a file.

I quickly dismissed (1) without attempting it, as that would require each
runner program be able to parse at least some subset of TOML and also
understand the benchmark definition directory structure. This easily eclipsed
my complexity threshold at this point.

My first actual attempt was (2), but that proved errant because some benchmark
definitions include many patterns. The "dictionary" benchmarks in particular
might search for thousands of literal words. Passing each one of those as
arguments to a sub-process quickly runs afoul of typical limits placed on the
size and number of those arguments. If it weren't for the limits, then this
approach would have worked fine. You wouldn't even need to bring in a "proper"
command line argument parser for this, since we'd only need to support a very
limited form that `rebar` specifically uses. Remember, the runner programs
just exist as a bridge for `rebar` to gather measurements. They don't *need* to
be user facing programs.

I eventually settled on (3) because it became obvious that neither the patterns
nor the haystack could be passed as process arguments due to their size. So,
some kind of format to encode the parameters would be needed. But which format?

A natural format to choose here would be JSON due to its ubiquitous support
in a variety of programming environments. But as mentioned in a previous
section, I wanted to keep runner programs as simple as I could. This doesn't
just include the source code itself, but also the operation, maintenance and
building of the programs too. For this reason, I wanted each runner program to
be "free of dependencies" to the extent possible. Since not all programming
environments come with a JSON parser built-in and not all programming
environments make it easy to add dependencies, I decided against JSON or any
other similarly complex format.

Instead, I came up with my own very simple format called [KLV](KLV.md), which
is an initialism for "key-length-value." It's essentially the standard
prefix-length encoding commonly found in binary formats, but done in a plain
text format. The plain text format makes the benchmark parameters human
readable. (I'm sure I didn't invent this format. What I mean is that there is
no specification, and I didn't copy the precise format from any other source.)

Thus, benchmark runner programs work by reading the entirety of `stdin`,
parsing it as a sequence of key-length-value entries, repeatedly running the
benchmark according to the parameters given and then finally outputting a list
of duration-and-count samples on `stdout`. The KLV format can be parsed in a
handful of lines in just about any programming language. The only primitives
you need are `memchr` (or "index of"), substring slicing and integer parsing.

Note that one complication with the KLV format is that it needs to be kept in
its raw byte representation. Converting it to, say, a Java `String` before
parsing it would be a mistake because the "length" in key-length-value is the
number of bytes in `value`. But if you convert it to a Java `String` first,
then since the length-in-bytes and the length of a Java `String` (UTF-16 code
units) aren't the same, it becomes more difficult to take a correct substring.
(Arguably, this implies that the KLV format is actually a binary format
masquerading as a plain text format. Or some hybrid of them.)

### Defining Benchmarks

Where as [KLV](KLV.md) represents the format of the input given to runner
programs, a [directory of TOML files](FORMAT.md) represents the format of the
input given to `rebar`. `rebar` is then responsible for translating this
format---along with other parameters---into KLV data, managing the execution of
sub-processes, gathering the samples produced and producing aggregate
statistics.

The problem we want to solve here is the ability to describe benchmarks in a
manner that is independent from the programs that run them. I chose TOML for
this because of a number of reasons:

* Rust has good support for it.
* It is, in my opinion, very easy to both read and write as a human.
* It supports comments, which is absolutely a requirement.
* It has several useful string literal types, which are particularly relevant
in a regex benchmark. For example, its single-quote string literals absolve us
of the need to deal with both TOML escape sequences and regex escape sequences.
Moreover, TOML's triple single-quote string literals are useful in cases where
the regex pattern contains a single-quote itself. (Patterns with three or more
consecutive single quotes would be inconvenient to deal with, but these are
rare enough to be non-existent.)

The TOML format used by `rebar` also provides facilities for including both
regex patterns and haystacks from other files. This avoids the need to put
exceptionally large regexes or haystacks inside the TOML file, which would make
reading the TOML files quite a bit more annoying than is necessary.

## Choosing Models

This regex barometer is a collection of micro-benchmarks because everything
that is measured is a pretty small well defined task. That is, the tasks being
measured are usually not representative of entire programs, but _parts_ of
programs. Even for programs like `grep` that specialize in running regex
searches, there are performance sensitive aspects of the program other than the
regex search.

The question we want to address here is: what tasks do we measure?

Most extant regex benchmarks, at the time of writing (2023-04-05), measure
only the most obvious thing to measure: the time it takes to find all
non-overlapping matches of a regex in a single haystack. While this is an
exceptionally common task for regexes, it is by no means the only common task.
There are several others:

* Extracting the sub-match positions from capture groups.
* Finding lines in some data that match a regex.
* Extracting the sub-match positions from capture groups for every line in some
data. The regex in this case is usually written in a way to match every line.
That is, the regex is used as an ad hoc parser for what is likely a very simple
and ad hoc format.
* The time it takes a compile a regex pattern.

Regex engines can vary quite a lot in the performance of each of these tasks.
Capturing measurements for all of these models tests the balance of latency
and throughput. For example, regex engines might do some work based on the
assumption that "no match" is the common case, but that work might hurt it in
cases where "every search is a match." It also tests the relative performance
difference between finding the overall match span of a regex and the individual
match spans of each capture group. The latter might not just require tracking
additional state, but might require running an entirely different regex engine
internally. This can add quite a bit of overhead when compared to regex engines
that can report sub-matches in a single scan.

The specific models used in this barometer are elaborated on in more detail in
[MODELS](MODELS.md). One thing worth mentioning though is that the rebar tool
itself does not have any foreknowledge about the specific models benchmarked.
The models are just an agreement between the benchmark definitions and the
runner programs about what to name a specific workload. Runner programs do not
need to support all models---indeed, not all regex engines support capture
groups---and so runner programs should return an error if they're called with
an unsupported model.

## Choosing Benchmarks

Benchmark selection is a critical part of evaluating not just the performance
of a single regex engine over time, but for evaluating the relative performance
between regex engines too. The design space for a regex engine implementation
is quite large, and there are many different strategies with their own
trade offs that can be employed.

It would be possible to construct a set of benchmarks where every measurement
favored a particular regex engine implementation strategy. For example, one
might choose a set of benchmarks where every pattern and haystack combination
exhibits catastrophic backtracking. Finite automata regex engines would end up
doing quite a bit better than backtracking regex engines on such a benchmark
set, because finite automata engines provide a worst case `O(m * n)` time
bound. But of course, this would not be a good general purpose regex barometer
because it doesn't do a good job of representing the diversity of real world
regex usage.

rebar doesn't quite fall into such an obvious trap as defining benchmarks that
consist purely of catastrophic backtracking, but it's still exceptionally
difficult to choose a representative set of benchmarks. Moreover,
[BIAS](BIAS.md) can also influence benchmark selection in a negative way.

So the question remains: how should benchmarks be chosen? In truth, this is
still a work-in-progress. I expect the "curated" set of benchmarks (the results
presented in the [README](README.md)) to evolve over time. Of course, this also
makes the results over time difficult to analyze if the thing we're measuring
changes. But the bottom line is that I don't expect to get the set of
benchmarks exactly right the first time.

With that said, I've started with a few guiding principles:

* Some benchmarks can be created from good sense. For example, it's plainly
clear that sometimes regexes are used to search for plain literals. So we can
add benchmarks that are just literals. And then add variations on that, for
example, alternations of literals and case insensitive searches for literals.
* We should endeavor to exercise both throughput and latency sensitive
workloads. Throughput workloads tend to correspond to regexes that rarely or
never match, where as latency workloads tend to correspond to regexes that
match very frequently. There can be a very dramatic difference between
throughput and latency since most regex engines do a non-trivial amount of work
before executing a search.
* Regexes used in "production" programs should also be represented. While there
is undoubtedly room for coming up with regexes from "good sense," it can be
quite difficult to know what the real world use cases are just from reasoning
alone. Real world use cases often exercise and stress things in creative ways
that are difficult to predict. Specific focus should be paid to regexes that
are purported bottlenecks in the larger program. These represent high-value
regexes that could potentially improve overall performance just by making the
regex engine faster.

These criteria are likely to evolve over time.

## Evaluating Measurements

The primary metric that rebar evaluates is time. That is, it measures how long
it takes to complete a certain task. In most cases, the thing being measured is
a kind of regex search. But rebar does come with a `compile` model where the
thing being measured is how long it takes to compile a regex.

There are other things that may be collected, but currently are not:

* Instruction count.
* Memory usage.

In theory, I would like to capture both of these things, but have not had the
time to dig into how I would go about it.

But let's go back to time. Merely collecting a bunch of timing samples is not
enough. Namely, a single regex search may be executed thousands (or more)
times. A human just cannot feasibly look at thousands of data points for every
regex engine across every workload. So instead, we need to choose a way to
represent a collection these data points in some aggregate form. There are a
number of choices:

* Arithmetic mean.
* Assume the distribution of timings is normal and use the arithmetic mean with
a standard deviation.
* Median.
* Minimum.
* Maximum.

rebar provides a way to use all of the above, but it principally represents
results using the median. The median is used because it is robust with respect
to outliers, and outliers do tend to occur when benchmarking regex engines. Not
only because of perhaps noise in the measurement process or the environment
itself, but also because many regex engines tend to start off more slowly than
where they end up. For example, Rust's regex crate uses a lazy DFA in many
cases, where some part of the DFA needs to be constructed at search time. After
a certain portion of the DFA is constructed, the search tends to acquire the
typical performance characteristics of a fully compiled DFA, but without the
associated downsides. However, it still needs to build some part of the DFA.
Whenever it needs to add a new transition (or state), then it will take much
longer than it would if the transition (or state) had already existed. A
similar concept might be applied to Java with its tracing JIT.

This turns out to be pretty good for looking at the results of a single
benchmark. But what if one wants to look at the results across many benchmarks?
Or perhaps all benchmarks? Rebar achieves this by using the geometric mean of
the speedup ratios for each benchmark. That is, for each benchmark and each
regex engine in that benchmark, a regex engine is assigned a speedup ratio.
That is, the amount it is slower than the fastest regex engine for that
benchmark. The collection of these speedup ratios for each regex engine is then
aggregated by taking its [geometric mean]. The geometric mean tends to be quite
robust against outliers, although not completely so. A regex engine that is the
fastest on every benchmark would have a geometric mean of `1.0`. Thus, a regex
engine with a geometric mean of `N` means that it is, on average, `N` times
slower than the fastest regex engine for any particular benchmark.

[geometric mean]: https://dl.acm.org/doi/pdf/10.1145/5666.5673
