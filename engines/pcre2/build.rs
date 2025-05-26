use std::env::var;

fn main() {
    let target = var("TARGET").unwrap();
    let upstream = std::path::PathBuf::from("upstream");

    // Set some config options. We mostly just use the default values. We do
    // this in lieu of patching config.h since it's easier.
    let mut builder = cc::Build::new();
    builder
        .define("PCRE2_CODE_UNIT_WIDTH", "8")
        .define("HAVE_STDLIB_H", "1")
        .define("HAVE_MEMMOVE", "1")
        .define("HEAP_LIMIT", "20000000")
        .define("LINK_SIZE", "2")
        .define("MATCH_LIMIT", "10000000")
        .define("MATCH_LIMIT_DEPTH", "10000000")
        .define("MAX_NAME_COUNT", "10000")
        .define("MAX_NAME_SIZE", "32")
        .define("NEWLINE_DEFAULT", "2")
        .define("PARENS_NEST_LIMIT", "250")
        .define("PCRE2_STATIC", "1")
        .define("STDC_HEADERS", "1")
        .define("SUPPORT_PCRE2_8", "1")
        .define("SUPPORT_UNICODE", "1");
    if target.contains("windows") {
        builder.define("HAVE_WINDOWS_H", "1");
    }
    enable_jit(&target, &mut builder);

    builder.include(upstream.join("src")).include(upstream.join("include"));
    for result in std::fs::read_dir(upstream.join("src")).unwrap() {
        let dent = result.unwrap();
        let path = dent.path();
        if path.extension().map_or(true, |ext| ext != "c") {
            continue;
        }
        // Apparently PCRE2 doesn't want to compile these directly, but only as
        // included from pcre2_jit_compile.c.
        //
        // ... and also pcre2_ucptables.c, which is included by pcre2_tables.c.
        // This is despite NON-AUTOTOOLS-BUILD instructions saying that
        // pcre2_ucptables.c should be compiled directly.
        if path.ends_with("pcre2_jit_match.c")
            || path.ends_with("pcre2_jit_misc.c")
            || path.ends_with("pcre2_ucptables.c")
        {
            continue;
        }
        builder.file(path);
    }

    if var("DEBUG").unwrap_or(String::new()) == "1" {
        builder.debug(true);
    }
    builder.compile("libpcre2.a");
}

// Historically, PCRE2 JIT compilation was problematic on Apple Silicon due to
// missing ___clear_cache symbol, but this appears to be resolved in current
// toolchains (tested successfully on macOS 14.6 with Xcode 15+).
//
// Previous linker errors on Apple platforms looked like:
//   Undefined symbols for architecture arm64:
//     "___clear_cache", referenced from:
//         _sljit_generate_code in libforeign.a(pcre2_jit_compile.o)
//   ld: symbol(s) not found for architecture arm64
//
// Previous issues documented:
// aarch64-apple-tvos         https://bugreports.qt.io/browse/QTBUG-62993
// aarch64-apple-darwin       https://github.com/Homebrew/homebrew-core/pull/57419
//
// JIT remains disabled for iOS/tvOS out of caution as these haven't been tested.
fn enable_jit(target: &str, builder: &mut cc::Build) {
    // Try enabling JIT on aarch64-apple-darwin (macOS) to see if the issue
    // has been resolved in newer toolchains. Keep it disabled for iOS/tvOS.
    if !target.contains("apple-ios") && !target.contains("apple-tvos") {
        builder.define("SUPPORT_JIT", "1");
    }
}
