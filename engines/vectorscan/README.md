This directory contains a Rust runner program for benchmarking [Vectorscan].
A fork of Intel's Hyperscan, modified to run on more platforms. Currently ARM NEON/ASIMD and Power VSX are 100% functional. ARM SVE2 support is in ongoing with access to hardware now. More platforms will follow in the future. Further more, starting 5.4.12 there is now a SIMDe port, which can be either used for platforms without official SIMD support, as SIMDe can emulate SIMD instructions, or as an alternative backend for existing architectures, for reference and comparison purposes.

The same assumptions as the Hyperscan engine are established.


[Vectorscan]: https://github.com/VectorCamp/vectorscan
[binding]: https://github.com/cosmicexplorer/spack-rs
