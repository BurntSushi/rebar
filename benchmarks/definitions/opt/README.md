This directory contains benchmarks that attempt to isolate and test *specific*
optimizations in regex engines.

Currently, the focus is on the `rust/regex` engine and I'm not sure whether I
want to expand beyond it. The idea is that these benchmarks might be able to
catch very specific regressions if they stop happening for some reason or
another.
