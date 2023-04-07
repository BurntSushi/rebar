fn main() {
    // Note that we need to link against both of these. uc contains all the
    // common goop, and i18n contains the regex engine.
    pkg_config::probe_library("icu-uc").unwrap();
    pkg_config::probe_library("icu-i18n").unwrap();
}
