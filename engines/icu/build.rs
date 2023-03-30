fn main() {
    // It's important this comes after compiling the shim, which results
    // in the correct order of arguments given to the linker.
    //
    // Note that we need to link against both of these. icu-uc contains all
    // the common goop, and i18n contains the regex engine.
    pkg_config::probe_library("icu-uc").unwrap();
    pkg_config::probe_library("icu-i18n").unwrap();
}
