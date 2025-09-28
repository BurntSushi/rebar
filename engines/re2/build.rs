use std::env::var;

const ABSEIL_DEPENDENCIES: &[&str] = &[
    "absl_base",
    "absl_core_headers",
    "absl_fixed_array",
    "absl_flags",
    "absl_flat_hash_map",
    "absl_flat_hash_set",
    "absl_inlined_vector",
    "absl_optional",
    "absl_span",
    "absl_str_format",
    "absl_strings",
    "absl_synchronization",
];

fn main() {
    let upstream = std::path::PathBuf::from("upstream");

    // If our shim layer changes, make sure Cargo sees it.
    println!("cargo:rerun-if-changed=binding.cpp");

    let mut builder = cc::Build::new();
    builder.cpp(true);
    builder.std("c++17");
    builder.include(&upstream);
    
    // Get Abseil include paths and flags before building
    // We need to collect all the include paths from all Abseil dependencies
    let mut seen_flags = std::collections::HashSet::new();
    for dep in ABSEIL_DEPENDENCIES {
        if let Ok(lib) = pkg_config::probe_library(dep) {
            for include_path in &lib.include_paths {
                builder.include(include_path);
            }
            // Also add any other flags that might be needed
            for flag in &lib.defines {
                let flag_str = format!("-D{}", flag.0);
                if seen_flags.insert(flag_str.clone()) {
                    builder.flag(&flag_str);
                }
            }
        }
    }
    
    // Currently compiling RE2 leads to a number of unused parameter warnings.
    // I'm not quite sure why, as the parameters are clearly being used for any
    // of the warnings I investigated. Maybe it's reflective of a more general
    // "dead code" lint? I'm not sure, but either way, we try to suppress them
    // here.
    builder.flag_if_supported("-Wno-unused-parameter");
    for result in std::fs::read_dir(upstream.join("re2")).unwrap() {
        let dent = result.unwrap();
        let path = dent.path();
        if path.extension().map_or(true, |ext| ext != "cc") {
            continue;
        }
        builder.file(&path);
        println!("cargo:rerun-if-changed={}", path.display());
    }
    for result in std::fs::read_dir(upstream.join("util")).unwrap() {
        let dent = result.unwrap();
        let path = dent.path();
        if path.extension().map_or(true, |ext| ext != "cc") {
            continue;
        }
        builder.file(&path);
        println!("cargo:rerun-if-changed={}", path.display());
    }
    // RE2 is a C++ library, so we need to compile our shim layer.
    builder.file("binding.cpp");

    if var("DEBUG").unwrap_or(String::new()) == "1" {
        builder.debug(true);
    }
    // Compile RE2 along with our binding in one go.
    builder.compile("libre2.a");

    // Note: We already probed for Abseil dependencies above and added their
    // include paths to the builder. The pkg_config::probe_library() calls
    // also set up the necessary linker flags automatically.
}
