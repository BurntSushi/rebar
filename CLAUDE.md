# Claude's Guide to Working with rebar

This document summarizes my experience working with the rebar regex benchmarking suite, including setup requirements, common issues, and tips for future work.

## Initial Setup

### Basic Installation
```bash
# Install rebar itself
cargo install --path .
```

### Python Engine Dependencies
The Python regex engines (`python/re` and `python/regex`) require `virtualenv` to be installed:
```bash
pip install virtualenv
```

This dependency is not documented in `BUILD.md` but is required for the Python engines to build successfully. Without it, you'll see errors like:
```
python/re: dependency command failed: failed to run command and wait for output
```

## Building Engines

### Successfully Built
- ✅ `rust/regex` - Works out of the box
- ✅ `python/re` - Works after installing virtualenv
- ✅ `python/regex` - Works after installing virtualenv  
- ✅ `pcre2` - Basic version works out of the box

### Build Issues Encountered

#### RE2
RE2 requires Abseil libraries. On macOS:
```bash
brew install abseil
```

However, even after installation, the build fails because the build.rs script doesn't properly pass the include paths from pkg-config to the C++ compiler. The precise error is:
```
fatal error: 'absl/base/macros.h' file not found
```

While pkg-config correctly returns the include path:
```bash
pkg-config --cflags absl_base
# Returns: -I/opt/homebrew/Cellar/abseil/20240722.1/include ...
```

The build.rs script calls pkg-config but doesn't add these include paths to the cc::Build configuration, so the compiler invocation lacks the necessary `-I` flags. This appears to be a bug in the RE2 engine's build configuration.

#### PCRE2 JIT
Initially failed with:
```
Error: JIT engine unavailable because JIT is not enabled
```

**Fixed by**: Modifying `engines/pcre2/build.rs` to enable JIT on macOS ARM64. The original build configuration disabled JIT on Apple Silicon due to historical linker issues with the `___clear_cache` symbol, but these appear to be resolved in current toolchains. After enabling JIT, PCRE2 performance improved by 3.4x to 11,600x (!), making it competitive with Rust regex.

## Running Benchmarks

### Basic Usage
```bash
# Build specific engines
rebar build -e '^(pcre2|rust/regex)$'

# Run benchmarks
rebar measure -e '^(pcre2|rust/regex)$' -f '^curated/01' > results.csv

# Compare results
rebar cmp results.csv -e '^rust/regex$' -e '^pcre2$'
```

### Performance Notes
- Some benchmark suites can take a long time to complete
- The `-f` flag is useful for filtering to specific benchmark groups
- Start with smaller benchmark sets (like `^curated/01`) for quick tests

## Key Findings
See `jq_regex_engine_comparison.md` for detailed benchmark results comparing PCRE2 and Rust regex performance.

## Outstanding Issues

1. **Documentation**: `BUILD.md` should mention the `virtualenv` dependency for Python engines
2. **RE2 Build**: Needs proper Abseil integration on macOS
3. **PCRE2 JIT**: Requires PCRE2 to be built with JIT support
4. **Timeout Command**: The `timeout` command is not available by default on macOS

## Tips for Future Work

1. **Check Dependencies First**: Use `RUST_LOG=debug rebar build -e '^engine_name$'` to see detailed error messages
2. **Start Small**: Run individual benchmark groups rather than the full suite
3. **Engine Configuration**: Check `benchmarks/engines.toml` for engine-specific build requirements
4. **Platform Differences**: Some engines may have platform-specific build requirements (especially on macOS vs Linux)

## Environment Details
- Platform: macOS (Darwin 23.6.0)
- Rust: cargo 1.87.0
- Python: 3.13.2 (via pyenv)
- Working directory: /Users/m/Documents/GitHub/rebar