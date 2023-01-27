use std::env::var;

fn main() {
    let upstream = std::path::PathBuf::from("upstream");

    // If our shim layer changes, make sure Cargo sees it.
    println!("cargo:rerun-if-changed=binding.cpp");

    let mut builder = cc::Build::new();
    builder.cpp(true);
    builder.include(&upstream);
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
    builder.compile("libre2.a");
}
