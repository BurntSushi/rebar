use crate::{new, Config};

pub(crate) fn run(c: &Config) -> anyhow::Result<Vec<timer::Sample>> {
    match &*c.engine {
        "nfa" => nfa(c),
        "meta" => meta(c),
        "dense" => dense(c),
        "sparse" => sparse(c),
        "hybrid" => hybrid(c),
        "backtrack" => backtrack(c),
        "pikevm" => pikevm(c),
        "onepass" => onepass(c),
        _ => unreachable!(),
    }
}

fn nfa(c: &Config) -> anyhow::Result<Vec<timer::Sample>> {
    use regex_automata::nfa::thompson::{pikevm::PikeVM, Compiler, NFA};
    use regex_syntax::ParserBuilder;

    let pattern = c.b.regex.one()?;
    let hir = ParserBuilder::new()
        .utf8(false)
        .unicode(c.b.regex.unicode)
        .case_insensitive(c.b.regex.case_insensitive)
        .build()
        .parse(&pattern)?;
    timer::run_and_count(
        &c.b,
        |nfa: NFA| {
            let re = PikeVM::builder().build_from_nfa(nfa)?;
            let mut cache = re.create_cache();
            Ok(re.find_iter(&mut cache, &c.b.haystack).count())
        },
        || Compiler::new().build_from_hir(&hir).map_err(|e| e.into()),
    )
}

fn meta(c: &Config) -> anyhow::Result<Vec<timer::Sample>> {
    timer::run_and_count(
        &c.b,
        |re: regex_automata::meta::Regex| {
            Ok(re.find_iter(&c.b.haystack).count())
        },
        || new::meta(c),
    )
}

fn dense(c: &Config) -> anyhow::Result<Vec<timer::Sample>> {
    timer::run_and_count(
        &c.b,
        |re: regex_automata::dfa::regex::Regex| {
            Ok(re.find_iter(&c.b.haystack).count())
        },
        || new::dense(c),
    )
}

fn sparse(c: &Config) -> anyhow::Result<Vec<timer::Sample>> {
    use regex_automata::dfa::{regex::Regex, sparse::DFA};
    timer::run_and_count(
        &c.b,
        |re: Regex<DFA<Vec<u8>>>| Ok(re.find_iter(&c.b.haystack).count()),
        || new::sparse(c),
    )
}

fn hybrid(c: &Config) -> anyhow::Result<Vec<timer::Sample>> {
    timer::run_and_count(
        &c.b,
        |re: regex_automata::hybrid::regex::Regex| {
            let mut cache = re.create_cache();
            Ok(re.find_iter(&mut cache, &c.b.haystack).count())
        },
        || new::hybrid(c),
    )
}

fn backtrack(c: &Config) -> anyhow::Result<Vec<timer::Sample>> {
    timer::run_and_count(
        &c.b,
        |re: regex_automata::nfa::thompson::backtrack::BoundedBacktracker| {
            let mut cache = re.create_cache();
            Ok(re.try_find_iter(&mut cache, &c.b.haystack).count())
        },
        || new::backtrack(c),
    )
}

fn pikevm(c: &Config) -> anyhow::Result<Vec<timer::Sample>> {
    timer::run_and_count(
        &c.b,
        |re: regex_automata::nfa::thompson::pikevm::PikeVM| {
            let mut cache = re.create_cache();
            Ok(re.find_iter(&mut cache, &c.b.haystack).count())
        },
        || new::pikevm(c),
    )
}

fn onepass(c: &Config) -> anyhow::Result<Vec<timer::Sample>> {
    use regex_automata::{
        dfa::onepass::DFA, util::iter::Searcher, Anchored, Input,
    };
    timer::run_and_count(
        &c.b,
        |re: DFA| {
            // The one-pass DFA only does anchored searches, so it doesn't
            // provide an iterator API. Technically though, we can still report
            // multiple matches if the regex matches are directly adjacent. So
            // we just build our own iterator.
            let mut cache = re.create_cache();
            let mut caps = re.create_captures();
            let input = Input::new(&c.b.haystack).anchored(Anchored::Yes);
            let it = Searcher::new(input)
                .into_matches_iter(|input| {
                    re.try_search(&mut cache, input, &mut caps)?;
                    Ok(caps.get_match())
                })
                .infallible();
            Ok(it.count())
        },
        || new::onepass(c),
    )
}
