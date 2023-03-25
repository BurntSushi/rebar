use crate::Config;

/// Constructor for regex-automata's "meta" regex engine.
pub(crate) fn meta(c: &Config) -> anyhow::Result<regex_automata::meta::Regex> {
    use regex_automata::meta::Regex;

    let config = Regex::config()
        // Disabling UTF-8 here just means that zero-width matches that split
        // a codepoint are allowed.
        .utf8_empty(false)
        .nfa_size_limit(Some((1 << 20) * 100));
    let re = Regex::builder()
        .syntax(syntax_config(c))
        .configure(config)
        .build_many(&c.b.regex.patterns)?;
    Ok(re)
}

/// Constructor for the fully compiled "dense" DFA.
pub(crate) fn dense(
    c: &Config,
) -> anyhow::Result<regex_automata::dfa::regex::Regex> {
    use regex_automata::{dfa::regex::Regex, nfa::thompson};

    let re = Regex::builder()
        .syntax(syntax_config(c))
        // Disabling UTF-8 here just means that zero-width matches that split
        // a codepoint are allowed.
        .thompson(thompson::Config::new().utf8(false))
        .build_many(&c.b.regex.patterns)?;
    Ok(re)
}

/// Constructor for the fully compiled "sparse" DFA. A sparse DFA is different
/// from a dense DFA in that following a transition on a state requires a
/// non-constant time lookup to find the transition matching the current byte.
/// In exchange, a sparse DFA uses less heap memory.
pub(crate) fn sparse(
    c: &Config,
) -> anyhow::Result<
    regex_automata::dfa::regex::Regex<
        regex_automata::dfa::sparse::DFA<Vec<u8>>,
    >,
> {
    use regex_automata::{dfa::regex::Regex, nfa::thompson};

    let re = Regex::builder()
        .syntax(syntax_config(c))
        // Disabling UTF-8 here just means that zero-width matches that split
        // a codepoint are allowed.
        .thompson(thompson::Config::new().utf8(false))
        .build_many_sparse(&c.b.regex.patterns)?;
    Ok(re)
}

/// Constructor for the hybrid NFA/DFA or "lazy DFA" regex engine. This builds
/// the underlying DFA at search time, but only up to a certain memory budget.
///
/// A lazy DFA, like fully compiled DFAs, cannot handle Unicode word
/// boundaries.
pub(crate) fn hybrid(
    c: &Config,
) -> anyhow::Result<regex_automata::hybrid::regex::Regex> {
    use regex_automata::{
        hybrid::{dfa::DFA, regex::Regex},
        nfa::thompson,
    };

    let re = Regex::builder()
        // This makes it so the cache built by this regex will be at least bit
        // enough to make progress, no matter how big it needs to be. This is
        // useful in benchmarking to avoid cases where construction of hybrid
        // regexes fail because the default cache capacity is too small. We
        // could instead just set an obscenely large cache capacity, but it
        // is actually useful to both see how the default performs and what
        // happens when the cache is just barely big enough. (When barely big
        // enough, it's likely to get cleared very frequently and this will
        // overall reduce search speed.)
        .syntax(syntax_config(c))
        // Disabling UTF-8 here just means that zero-width matches that split
        // a codepoint are allowed.
        .thompson(thompson::Config::new().utf8(false))
        .dfa(DFA::config().skip_cache_capacity_check(true))
        .build_many(&c.b.regex.patterns)?;
    Ok(re)
}

/// Constructor for the PikeVM, which can handle anything including Unicode
/// word boundaries and resolving capturing groups, but can be quite slow.
pub(crate) fn pikevm(
    c: &Config,
) -> anyhow::Result<regex_automata::nfa::thompson::pikevm::PikeVM> {
    use regex_automata::nfa::thompson::{self, pikevm::PikeVM};

    let re = PikeVM::builder()
        .syntax(syntax_config(c))
        // Disabling UTF-8 here just means that zero-width matches that split
        // a codepoint are allowed.
        .thompson(thompson::Config::new().utf8(false))
        .build_many(&c.b.regex.patterns)?;
    Ok(re)
}

/// Constructor for the bounded backtracker. Like the PikeVM, it can handle
/// Unicode word boundaries and resolving capturing groups, but only works on
/// smaller inputs/regexes. The small size is required because it keeps track
/// of which byte/NFA-state pairs it has visited in order to avoid re-visiting
/// them. This avoids exponential worst case behavior.
///
/// The backtracker tends to be a bit quicker than the PikeVM.
pub(crate) fn backtrack(
    c: &Config,
) -> anyhow::Result<regex_automata::nfa::thompson::backtrack::BoundedBacktracker>
{
    use regex_automata::nfa::thompson::{self, backtrack::BoundedBacktracker};

    let re = BoundedBacktracker::builder()
        .syntax(syntax_config(c))
        // Disabling UTF-8 here just means that zero-width matches that split
        // a codepoint are allowed.
        .thompson(thompson::Config::new().utf8(false))
        .build_many(&c.b.regex.patterns)?;
    Ok(re)
}

/// Constructor for the one-pass DFA, which can handle anything including
/// Unicode word boundaries and resolving capturing groups, but only works on a
/// specific class of regexes known as "one-pass." Moreover, it can only handle
/// regexes with at most a small number of explicit capturing groups.
pub(crate) fn onepass(
    c: &Config,
) -> anyhow::Result<regex_automata::dfa::onepass::DFA> {
    use regex_automata::{dfa::onepass::DFA, nfa::thompson};

    let re = DFA::builder()
        .syntax(syntax_config(c))
        // Disabling UTF-8 here just means that zero-width matches that split
        // a codepoint are allowed.
        .thompson(thompson::Config::new().utf8(false))
        .build_many(&c.b.regex.patterns)?;
    Ok(re)
}

/// For regex-automata based regex engines, this builds a syntax configuration
/// from a benchmark definition.
pub(crate) fn syntax_config(
    c: &Config,
) -> regex_automata::util::syntax::Config {
    regex_automata::util::syntax::Config::new()
        // Disabling UTF-8 just makes it possible to build regexes that won't
        // necessarily match UTF-8. Whether Unicode is actually usable or not
        // depends on the 'unicode' option.
        .utf8(false)
        .unicode(c.b.regex.unicode)
        .case_insensitive(c.b.regex.case_insensitive)
}
