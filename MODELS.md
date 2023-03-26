# Benchmark Models

This regex barometer defines a number of different benchmark models. Each
benchmark model is generally intended to correspond to a "real world" regex
task. Whether that be searching line-by-line like a grep or log searching tool,
or searching large haystacks all at once (perhaps memory mapped from disk), or
even just how long it takes to compile a regex.

The purpose of defining distinct models is to attempt to capture a broad range
of regex tasks that represent or approximate real world workloads. _Most_
regex benchmarks either define only one model, or even worse, combine multiple
models together. Usually the only model measured by other regex benchmarks is
"how many times does a regex match in a haystack." But real world tasks often
involve extracting the positions that capturing groups match, and that task
usually requires asking the regex engine to do more work.

A secondary purpose of defining distinct models is that many regex engines
contain a number of different optimization techniques. Those optimization
techniques may or may not apply depending on the workload. For example, Rust's
regex crate might use a bounded backtracking algorithm to resolve capturing
groups when searching short haystacks, but might otherwise not use this
technique at all. Therefore, such an optimization is more likely to show up
when resolving capturing groups while, say, searching the lines in a log file,
but is less likely to show up when searching a document that is megabytes
big all at once. A benchmark that doesn't include varied models will miss an
opportunity to capture this nuance.

Of course, all models are wrong, but some are useful. Most regex benchmarks
never set out to be exhaustive, and certainly this set of models is not
exhaustive either. Balance is important. A single model is almost certainly the
wrong balance to strike.

What follows is a description of each model and its motivation for existing.

## `compile`

This model measures the compile time of a regex. Implementations of this model
_only_ measure the time it takes to build a regex to the point that it is ready
for a search. Implementations must also execute the regex on a haystack and
report the number of matches as a verification step. This verification step
is explicitly not measured as part of the timing of this benchmark, but if an
incorrect count is reported, the measurement for that specific regex engine
will fail.

This model is important because regex compilation sometimes matters. While not
all programmers are careful about making sure regex compilation happens no
more than it needs to, many programmers are used to the idea that one should
try to compile a regex just once if they can, and then reuse that same regex
for all searches. Namely, re-compiling the same regex in a loop is usually a
performance footgun.

Some higher level environments, such as Python's `re` module, will actually
cache the compilation of regexes such that one doesn't usually need to care
about when regex compilation happens at all. Instead, "just compile each regex
once" happens automatically without users of the regex library caring about
it. However, most lower level regex engine APIs do not include this sort of
automatic caching behavior.

Since it is generally expected that compiling a regex will take a longer time
than executing a search with that regex (for some reasonable haystack length),
and since many are used to the notion of building a regex once and reusing
it for the lifetime of a program, it follows that the compilation time of a
regex is *usually* not a major concern. With that said, this is only true if
compilation time is *reasonable*. For example, if a regex engine uses fully
compiled DFAs for all regexes, then it would not be hard (especially with large
Unicode character classes) to blow this "reasonableness" budget to the point
that a single regex might take multiple seconds to compile. (Or perhaps much
more, since DFAs take `O(2^n)` time to build where `n ~ len(pattern)`.)

Another reason regex compilation might matter is if the regex is being compiled
in a context where a server is responding to a client request. In this case,
the regex might be compiled once and only used once, such that compilation
time, if it takes a long time, could have a material impact on the absolute
response time of the request.

Overall, it is important to have some kind of barometer as to what compilation
times look like for a particular regex engine. While the most obvious question
is just a coarse, "are they reasonable," there are other use cases where
compilation time might matter more.

This is currently the only model that measures compilation time of a regex.
(Except for perhaps `regex-redux`, but in that case, the regexes are simple
enough and the haystack is big enough that compilation time doesn't factor into
it so long as it's reasonable.) Namely, in most benchmarks, search times are
so fast that if they included compilation time, then compilation time would
dominate and the signal from search time benchmarks would be greatly diminished
or snuffed out completely.

## `count`

The `count` model resembles what _most_ other regex benchmarks do: it measures
the time it takes to find all matches in a single haystack. The verification
step simply confirms that the number of matches corresponds to what is
expected.

Like most other regex benchmarks (but not all), this does _not_ include the
time it takes to compile the regex.

This model is the "main" barometer of regex engine speed. It is especially
good at measuring throughput (for large haystacks) and latency (for very short
haystacks). If you only looked at one model in this regex barometer, this would
be the one to look at.

Implementations of this model may use whatever techniques available to them
to compute the total count of matches. For example, this might mean avoiding
tracking capture group spans in a backtracker, or even avoiding finding the
start of a match in automata oriented engines.

## `count-spans`

This model is like `count`, except it returns a sum of the lengths of all
matches found in a single haystack. The verification step simply confirms that
the sum matches what is expected.

The length of a match should ideally be in terms of the number of bytes, but
it is also permissible to count the number of code units. For example, .NET's
regex engine can only run on sequences of UTF-16 code units, so using a length
derived from anything other than UTF-16 code units implies an overhead cost
that would otherwise be artificial to this benchmark. Benchmark definitions
will need to account for this by specifying different counts expected for regex
engines that count something other than individual bytes.

For example, given the regex `[0-9]{2}|[a-z]` and the haystack `12a!!34`, the
total sum reported should be `len(12) + len(a) + len(34) = 5`.

The purpose of this model is to force regex engines to compute the full bounds
of a match, which includes both the start and end of the match. A regex engine
may internally use any technique for reporting the bounds of a match. The
important bit is that implementations of this model _ask_ the regex engine for
both the start and end of each match, and then sum the lengths of every match.

Having both `count` and `count-spans` models somewhat complicates the overall
barometer, but the inclusion of Hyperscan in this barometer nearly demands
that we do so. Namely, Hyperscan can have quite different performance
characteristics depending on whether the start of the match is requested from
it or not. Moreover, Hyperscan seems to have more stringent limits on the size
of regexes it allows when the start of match is requested. For this reason,
it's quite important to ensure we have a defined and supported way of modeling
the performance of regex engines on tasks that do not require the start of a
match.

## `count-captures`

This model is like `count`, but instead of counting the number of matches,
it counts the number of matching capturing groups. For example, given the
regex `([0-9])([0-9])|([a-z])` and the haystack `12a34`, the total number of
matching capturing groups is `8`. Namely, there are 3 matches:

* `12` matches the first alternate, which contributes 1 implicit group (the
overall match) and 2 explicit groups.
* `a` matches the second alternate, which contributes 1 implicit group and
1 explicit group.
* `34` matches the first alternate, which contributes 1 implicit group and 2
explicit groups.

That adds up to `1 + 2 + 1 + 1 + 1 + 2 = 8`.

The important bit to notice here is that not all capturing groups necessarily
participate in a match. So it isn't always enough to simply report `1 +
number-of-capturing-groups-in-pattern` for every match. The regex engine needs
to actually compute which capturing groups participate in the match.

The purpose of this model is to measure timings for a very common regex task,
but one that can have a very different performance profile from the `count`
model. Namely, some regex engines need to do a lot of additional work to
report the spans for all capture groups that participate in a match. So even
if a regex engine appears fast in the `count` model, it could be quite a bit
slower in the `count-captures` model. If your primary use case includes using
capturing groups in your regex, then the `count` model could give quite a
misleading impression.

To simplify implementations of the model, one may assume that any regex
measured under this model will never match the empty string. In particular, it
means that one can iterate over all successive matches in a haystack with very
simple code:

```
count = 0
at = 0
loop:
    captures = re.find_at(haystack, at)
    if not captures.is_match():
        break
    for capture in captures:
        if capture.is_match():
            count += 1
    # If one had to handle the case where
    # at == captures.end() (i.e., the match
    # was empty), then this would lead to
    # an infinite loop.
    at = captures.end()
```

Note that the `count` benchmark model must handle the case of an empty match
correctly. If it's simpler to use a regex engine's iterator that handles the
empty match for you, then it's acceptable to use it here in this benchmark
model. (Its cost is likely neglible, although one may test that empirically and
use a simpler iteration protocol like the one above if that assumption proves
to be false.)

## `grep`

This model measures the time it takes to iterate over every line in a haystack
and report the number of such lines that contain at least one match of the
regex. The verification step compares the total number of matching lines, and
not the total number of matches. So for example, a line that contains 3 matches
of a regex will only contribute 1 to the overall count.

Approximate pseudo code for the benchmark looks like this. The comments explain
some of the particulars:

```
regex = ...
haystack = ...
count = 0
while not haystack.is_empty():
  line_end = haystack.find('\n')
  if line_end == -1:
    # If no more line terminators could be found,
    # then the entire remaining portion of the haystack
    # corresponds to the last line.
    line_end = haystack.len()
  line = haystack[0..line_end]
  haystack = haystack[line_end..haystack.len()]
  # This handles CRLF. If the line was terminated by \r\n,
  # then we strip the \r too.
  if not line.is_empty() and line[line.len()-1] == '\r':
    line = line[0..line.len()-1]
  # The regex engine shouldn't be given the line terminator.
  if regex.is_match(line):
    count += 1
print(count)
```

In this model, line iteration is actually included as part of the measurement.
Including line iteration in the measurement both simplifies the model and
more closely reflects reality. For example, if you can't separate Python's
`re` regex engine from Python itself, and you need to accomplish the task of
grepping a log file, then line iteration _should_ be part of what's measured
because it most closely resembles the real world task.

The purpose of this model is to capture a very common use case: iterate over
the lines of a file and do "something" with lines that match some regex
pattern. In this model, we leave out the "something" and instead just measure
how long it takes to find the set of lines that match a regex pattern. This of
course resembles the workload of the venerable `grep` tool, hence the name of
the model.

Note that "fast" implementations of grep tend to not work by splitting the
haystack into lines and then running a search on each line. Instead, they
split the haystack into very large chunks and then run a regex search on the
chunk instead of each line individually. This approach is taken primarily for
performance, so why don't we use that model here? For a few reasons:

1. It's not what is typically done for ad hoc simplistic re-implementations of
grep. In my experience, when you need to find matching lines in a haystack, you
just iterate over the lines and run the regex.
2. The model is much more complex. It requires somewhat careful buffer
management and also some minor sophistication with the regex itself. Namely, in
order for this technique to work, you need to guarantee that the regex does not
match `\n` anywhere, or else it might report matches that span multiple lines,
which would be an incorrect result for the high level task here. This means,
for example, you need to rewrite regexes that contain `\s` to instead use a
character class that lacks `\n`.

Thus, we stick with a very simple model that also has the benefit of reflecting
real world use cases.

## `grep-captures`

This model is similar to `grep` in that it works by iterating over every line
in a haystack and searching it with a regex, except instead of reporting the
number of matching lines it reports the total number of matching capturing
groups. This includes multiple matches within the same line.

Approximate pseudo code for the benchmark looks like this. The comments explain
some of the particulars:

```
regex = ...
haystack = ...
count = 0
while not haystack.is_empty():
  line_end = haystack.find('\n')
  if line_end == -1:
    # If no more line terminators could be found,
    # then the entire remaining portion of the haystack
    # corresponds to the last line.
    line_end = haystack.len()
  line = haystack[0..line_end]
  haystack = haystack[line_end..haystack.len()]
  # This handles CRLF. If the line was terminated by \r\n,
  # then we strip the \r too.
  if not line.is_empty() and line[line.len()-1] == '\r':
    line = line[0..line.len()-1]
  # The regex engine shouldn't be given the line terminator.
  # Remember, we need to find all matches within a line.
  for captures in regex.captures(line):
    for group in captures.groups():
      # Not all groups necessarily participate in a match!
      # So this must only include groups that match.
      if group is not None:
        count += 1
print(count)
```

The purpose of this model is very similar to `grep`, but measures a task that
usually requires more work. That is, while "just finding matching lines" is a
common task unto itself, it is _also_ common to not just find matching lines
but pluck actual data out of each line. For example, you might use a regex to
parse each line in an HTTP server log, and use capturing groups to extract
pieces of data like the HTTP response code, referrer and other things.

Just as with `count-captures`, this model is useful because reporting the spans
of matching capturing groups usually requires more work than simply answering
whether a line matches or not. Given how common this sort of task is, it's
important to measure it as a distinct model.

Also as with `count-captures`, implementations of this model may assume
that the regex being measured will never match the empty string. See the
`count-captures` model for more details.

## `regex-redux`

This is a port of the [regex-redux benchmark][regex-redux] from [The Benchmark
Game][benchgame]. Unlike most other regex benchmarks, `regex-redux` is more of
a holistic benchmark that doesn't focus each unit of measurement on a single
regex, but rather, a task that requires compiling and searching for many
different regexes.

Implementations of this model *must* check that their output is equivalent
to the following string:

```
agggtaaa|tttaccct 6
[cgt]gggtaaa|tttaccc[acg] 26
a[act]ggtaaa|tttacc[agt]t 86
ag[act]gtaaa|tttac[agt]ct 58
agg[act]taaa|ttta[agt]cct 113
aggg[acg]aaa|ttt[cgt]ccct 31
agggt[cgt]aa|tt[acg]accct 31
agggta[cgt]a|t[acg]taccct 32
agggtaa[cgt]|[acg]ttaccct 43

1016745
1000000
547899
```

Implementations must also report the length of the input at the end of
execution (after all replacements have been made), and this is checked against
the `count` field in the benchmark definition.

This is probably the weakest model in this particular regex barometer, but it
is included due to the popularity of The Benchmark Game. It provides a way to
connect the results for regex engines in this benchmark with the results in a
different benchmark.

Do note though that the benchmarks aren't precisely equivalent. There are at
least the following differences:

* Parallelism is forbidden.
* Unicode is disabled for all regex engines that participate since the
benchmark definition from The Benchmark Game does not require any sort of
Unicode support. Disabling Unicode support in the regex engines makes this more
of an apples-to-apples comparison.
* The haystack used is smaller than the haystack used by the Benchmark Game.
This makes running the benchmark faster, but the important bit is that the
timings are not directly comparable.

This model embeds its own regexes, so the `regex` field must be empty, i.e.,
`regex = []`.

[regex-redux]: https://benchmarksgame-team.pages.debian.net/benchmarksgame/description/regexredux.html
[benchgame]: https://benchmarksgame-team.pages.debian.net/benchmarksgame/
