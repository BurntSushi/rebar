macro_rules! regex {
    ($re:literal $(,)?) => {{
        use regex_lite::Regex;
        use std::sync::OnceLock;

        static RE: OnceLock<Regex> = OnceLock::new();
        RE.get_or_init(|| Regex::new($re).unwrap())
    }};
}
