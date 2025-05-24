# Fresh Regex Engine Comparison for jq Migration

Based on benchmarks run with rebar on 2025-05-24.

## Summary of Results

### Literal Search Performance (curated/01-literal)

| Benchmark | PCRE2 | PCRE2/JIT | RE2 | Rust regex | Best Performance |
|-----------|-------|-----------|-----|------------|------------------|
| sherlock-en | 4.0 GB/s | **13.7 GB/s** | 6.3 GB/s | **19.2 GB/s** | Rust (1.40x faster than JIT) |
| sherlock-casei-en | 451.4 MB/s | **6.6 GB/s** | 2.0 GB/s | 5.4 GB/s | PCRE2/JIT (1.23x faster than Rust) |
| sherlock-ru | 1.9 MB/s | **22.1 GB/s** | 316.0 MB/s | **23.4 GB/s** | Rust (1.06x faster than JIT) |
| sherlock-casei-ru | 1.9 MB/s | **9.2 GB/s** | 424.3 MB/s | 5.6 GB/s | PCRE2/JIT (1.66x faster than Rust) |
| sherlock-zh | 49.0 MB/s | **28.3 GB/s** | 913.5 MB/s | 27.1 GB/s | PCRE2/JIT (1.05x faster than Rust) |

## Key Findings

1. **PCRE2 JIT is a game-changer**: Enabling JIT compilation improves PCRE2's performance by 3.4x to 11,600x (!), making it competitive with Rust regex.

2. **Performance is now very close**: With JIT enabled, PCRE2 and Rust regex trade wins:
   - Rust regex is slightly faster for basic literal matching
   - PCRE2/JIT is faster for case-insensitive matching
   - Both handle Unicode text excellently with JIT enabled

3. **RE2 lags behind**: While respectable, RE2 is consistently slower than both PCRE2/JIT and Rust regex in these benchmarks.

4. **The Unicode issue is resolved**: PCRE2's terrible performance on non-ASCII text was due to lack of JIT compilation. With JIT enabled, it matches Rust's excellent Unicode performance.

## Updated Recommendations for jq

1. **PCRE2 with JIT is now the clear choice** for jq migration:
   - Performance is competitive with Rust regex (within 2x for all tests)
   - Offers Oniguruma compatibility features for easier migration
   - Mature, stable, and actively maintained
   - Excellent Unicode support with JIT enabled
   - Better feature compatibility with jq's current regex needs

2. **Rust regex** would still be excellent for performance, but:
   - Would require more significant changes to jq's codebase
   - Lacks some advanced features that Oniguruma/PCRE2 support (backreferences, lookbehind, etc.)
   - These features may be used in existing jq scripts

3. **RE2** appears less suitable given its performance lag and different feature set

## Build Notes

- PCRE2 JIT was successfully enabled on Apple Silicon macOS by modifying the build.rs file
- The original build configuration disabled JIT on ARM64 macOS due to historical linker issues
- These issues appear to be resolved in current toolchains

## Further Testing Recommendations

To make a final decision for jq, it would be valuable to test:
1. More complex regex patterns (not just literals)
2. Capture group performance (important for jq's regex extraction)
3. Unicode property matching (e.g., `\p{Letter}`)
4. Backreference support (if used in existing jq scripts)
5. Compilation time for complex patterns
6. Memory usage comparison

## How to Run More Benchmarks

```bash
# Build all engines
rebar build -e '^(pcre2|re2|rust/regex)$'

# Run comprehensive benchmarks
rebar measure -e '^(pcre2|rust/regex)$' -f '^curated/' > results.csv

# Compare results
rebar cmp results.csv -e '^rust/regex$' -e '^pcre2$'
```