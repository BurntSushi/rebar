This directory contains saved sets of recorded measurements. We generally try
to record measurements for each regex engine in their own CSV file for maximum
flexibility.

We record measurements like this so that we can track the performance of each
regex engine over time.

The `all` directory contains measurements recorded for every benchmark
definition and regex engine. This makes it possible to explore measurements for
each regex engine without having to spend the time to record them yourself.
(Which takes hours to do for all benchmarks.)

See the [TUTORIAL](../../TUTORIAL.md) for a guide on how to explore a large set
of measurements using `rebar`.
