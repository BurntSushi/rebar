This directory contains tests masquerading as benchmark definitions. That is,
the benchmarks in this directory are not used for gathering measurements, but
for checking what regex engines and the rebar runner programs actually support.

This is not meant to be a completely exhaustive set of tests for supported
regex features, but more are definitely welcome if you find a big gap. A real
regex test suite will usually have a lot more stuff than what is here, but this
serves as a way of approximating and actually seeing some of the differences
between regex engines.
