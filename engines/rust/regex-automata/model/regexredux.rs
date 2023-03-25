use crate::{new, Config};

pub(crate) fn run(c: &Config) -> anyhow::Result<Vec<timer::Sample>> {
    match &*c.engine {
        "meta" => meta(c),
        "dense" => dense(c),
        "hybrid" => hybrid(c),
        "pikevm" => pikevm(c),
        _ => unreachable!(),
    }
}

fn meta(c: &Config) -> anyhow::Result<Vec<timer::Sample>> {
    use regex_automata::meta::Regex;

    let haystack = c.b.haystack_str()?;
    let compile = |pattern: &str| -> anyhow::Result<regexredux::RegexFn> {
        let re = Regex::builder()
            .syntax(new::syntax_config(c))
            .configure(Regex::config().utf8_empty(false))
            .build(pattern)?;
        let find = move |h: &str| Ok(re.find(h).map(|m| (m.start(), m.end())));
        Ok(Box::new(find))
    };
    timer::run(&c.b, || regexredux::generic(haystack, compile))
}

fn dense(c: &Config) -> anyhow::Result<Vec<timer::Sample>> {
    use regex_automata::{dfa::regex::Regex, nfa::thompson};

    let haystack = c.b.haystack_str()?;
    let compile = |pattern: &str| -> anyhow::Result<regexredux::RegexFn> {
        let re = Regex::builder()
            .syntax(new::syntax_config(c))
            .thompson(thompson::Config::new().utf8(false))
            .build(pattern)?;
        let find = move |h: &str| -> anyhow::Result<Option<(usize, usize)>> {
            Ok(re.find(h).map(|m| (m.start(), m.end())))
        };
        Ok(Box::new(find))
    };
    timer::run(&c.b, || regexredux::generic(haystack, compile))
}

fn hybrid(c: &Config) -> anyhow::Result<Vec<timer::Sample>> {
    use regex_automata::{
        hybrid::{dfa::DFA, regex::Regex},
        nfa::thompson,
    };

    let haystack = c.b.haystack_str()?;
    let compile = |pattern: &str| -> anyhow::Result<regexredux::RegexFn> {
        let re = Regex::builder()
            .syntax(new::syntax_config(c))
            .thompson(thompson::Config::new().utf8(false))
            .dfa(DFA::config().skip_cache_capacity_check(true))
            .build(pattern)?;
        let mut cache = re.create_cache();
        let find = move |h: &str| -> anyhow::Result<Option<(usize, usize)>> {
            Ok(re.find(&mut cache, h).map(|m| (m.start(), m.end())))
        };
        Ok(Box::new(find))
    };
    timer::run(&c.b, || regexredux::generic(haystack, compile))
}

fn pikevm(c: &Config) -> anyhow::Result<Vec<timer::Sample>> {
    use regex_automata::{
        nfa::thompson::{self, pikevm::PikeVM},
        util::captures::Captures,
    };

    let haystack = c.b.haystack_str()?;
    let compile = |pattern: &str| -> anyhow::Result<regexredux::RegexFn> {
        let re = PikeVM::builder()
            .syntax(new::syntax_config(c))
            .thompson(thompson::Config::new().utf8(false))
            .build(pattern)?;
        let mut cache = re.create_cache();
        let mut caps = Captures::matches(re.get_nfa().group_info().clone());
        let find = move |h: &str| -> anyhow::Result<Option<(usize, usize)>> {
            re.captures(&mut cache, h, &mut caps);
            Ok(caps.get_match().map(|m| (m.start(), m.end())))
        };
        Ok(Box::new(find))
    };
    timer::run(&c.b, || regexredux::generic(haystack, compile))
}
