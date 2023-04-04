This document describes the methodology of this regex barometer. In particular,
we go over the motivation for the problem this barometer tries to solve and its
high level design.

For details on the specific tasks we measure, see the [MODELS](MODELS.md)
document. This document instead tries to describe things at a higher level,
and in particular how the rebar harness works.

## The Problem

Speaking personally, there were two primary reasons for why I developed this
barometer:

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

### Rust regex crate performance

My estimation is that tracking performance of the regex crate benefits from the
following things:

* Flexibility for measuring varying workloads. So for example, we shouldn't be
limited to just counting the number of times a regex matches. We should also
measure the time it takes to find the positions matched by each capture group.
I also wanted to include a workload that iterates over the lines of a file and
runs a regex on each line, or in other words, a simple grep-like program since
it's such a common workload.
* It should be very easy to add new benchmarks. Benchmarks don't just come from
the regexes that I think are interesting, they also come from looking at how
regexes are used in programs and also from user reports of slow search times.
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

### Measuring other regex engines

Measuring other regex engines presents its own unique challenged, but I believe
the following to be useful properties:

* Measurements should reflect "real world" use cases as much as possible. In
particular, this means not adding extra overhead just for the convenience of
benchmarking.
* The work required to add a new regex engine to the benchmark harness should
be small and simple. When given the choice between a big amount of work and
complex work, we should prefer more work over complex work.
* The benchmarks themselves should be defined in a common format that works
across all supported regex engines.
* Comparisons between engines should generally be as "apples to apples" as
possible. This means that gathering measurements should try to both model
real world workloads while also trying to keep the actual work being performed
as similar as possible.

## Design: process oriented architecture

When I first started rebar (before it was even called rebar), I had scoped the
project to just the regex crate. That is, I only cared about the first half of
the problem described above. But then I started getting curious about how other
regex engines performed on similar tasks. An especially, I wanted to learn
about optimization techniques that other regex engines used that I might be
able to port.

So I started adding other regex engines to the predecessor of rebar. I started
with only the PCRE2 and RE2 regex engines, which I accomplished by binding them
through a C API and capturing measurements from Rust code. At the time, I told
myself that I would just limited myself to regex engines that could be
reasonably called through a C API without cost.

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

* rebar no longer knows about any specific regex engine that it measures. That
is, rebar doesn't bundle regex engines like RE2 or PCRE2. Instead, all
measurements are captured over a process boundary.
* The programs that execute a benchmark have to _also_ be responsible for
timing those measurements. I at first resisted this, because I wanted this
sort of thing to be under the control of a single piece of code that remains
the same across all measurements. But this is somewhat tricky to do reliably.
We can't rely on the time it takes to execute a process since many times are
measured in nano or micro seconds. The overhead of process creation would make
capturing such timings infeasible. Thankfully, most programming environments
these days provide a way to capture high resolution timings.
* The programs need to report a count from each benchmark execution back to
rebar, and rebar should verify that this count is what is expected. This is
critical for ensuring that the workload is what you expect it to be.
* rebar should otherwise be responsible for the aggregation of timings and
other analyses. That is, the runner programs just need to run some code
repeatedly and collect a sample for each execution. The sample consists of the
time it took to execute and the count returned by it. Then, once "enough"
samples have been collected, they should be sent back to rebar.

## Design: a simple format for describing benchmark execution
