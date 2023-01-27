This is a small Rust library that implements a generic version of the
regex-redux benchmark. This is useful because there are a number of benchmark
harness programs written in Rust (even for testing regex engines written in C),
and it's useful to just have this code written once.

This exposes `verify` and `generic` functions. The former checks that the
output of `generic` is correct and the latter runs the benchmark itself.
