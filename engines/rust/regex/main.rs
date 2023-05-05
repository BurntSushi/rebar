use std::io::Write;

use {
    anyhow::Context,
    bstr::ByteSlice,
    lexopt::Arg,
    // See README for why we use regex-automata instead of regex.
    regex_automata::{meta::Regex, Input},
};

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let mut p = lexopt::Parser::from_env();
    let (mut quiet, mut version) = (false, false);
    while let Some(arg) = p.next()? {
        match arg {
            Arg::Short('h') | Arg::Long("help") => {
                anyhow::bail!("main [--version | --quiet]")
            }
            Arg::Short('q') | Arg::Long("quiet") => {
                quiet = true;
            }
            Arg::Long("version") => {
                version = true;
            }
            _ => return Err(arg.unexpected().into()),
        }
    }
    if version {
        writeln!(std::io::stdout(), "{}", env!("CARGO_PKG_VERSION"))?;
        return Ok(());
    }
    let b = klv::Benchmark::read(std::io::stdin())
        .context("failed to read KLV data from <stdin>")?;
    let samples = match b.model.as_str() {
        "compile" => model_compile(&b)?,
        "count" => model_count(&b, &compile(&b)?)?,
        "count-spans" => model_count_spans(&b, &compile(&b)?)?,
        "count-captures" => model_count_captures(&b, &compile(&b)?)?,
        "grep" => model_grep(&b, &compile(&b)?)?,
        "grep-captures" => model_grep_captures(&b, &compile(&b)?)?,
        "regex-redux" => model_regex_redux(&b)?,
        _ => anyhow::bail!("unrecognized benchmark model '{}'", b.model),
    };
    if !quiet {
        let mut stdout = std::io::stdout().lock();
        for s in samples.iter() {
            writeln!(stdout, "{},{}", s.duration.as_nanos(), s.count)?;
        }
    }
    Ok(())
}

fn model_compile(b: &klv::Benchmark) -> anyhow::Result<Vec<timer::Sample>> {
    let haystack = &*b.haystack;
    timer::run_and_count(
        b,
        |re: Regex| Ok(re.find_iter(haystack).count()),
        || compile(b),
    )
}

fn model_count(
    b: &klv::Benchmark,
    re: &Regex,
) -> anyhow::Result<Vec<timer::Sample>> {
    let haystack = &*b.haystack;
    timer::run(b, || Ok(re.find_iter(haystack).count()))
}

fn model_count_spans(
    b: &klv::Benchmark,
    re: &Regex,
) -> anyhow::Result<Vec<timer::Sample>> {
    let haystack = &*b.haystack;
    timer::run(b, || Ok(re.find_iter(haystack).map(|m| m.len()).sum()))
}

fn model_count_captures(
    b: &klv::Benchmark,
    re: &Regex,
) -> anyhow::Result<Vec<timer::Sample>> {
    let mut input = Input::new(&*b.haystack);
    let mut caps = re.create_captures();
    timer::run(b, || {
        input.set_start(0);
        let mut count = 0;
        while let Some(m) = {
            re.search_captures(&input, &mut caps);
            caps.get_match()
        } {
            for i in 0..caps.group_len() {
                if caps.get_group(i).is_some() {
                    count += 1;
                }
            }
            // Benchmark definition says we may assume empty matches are
            // impossible.
            input.set_start(m.end());
        }
        Ok(count)
    })
}

fn model_grep(
    b: &klv::Benchmark,
    re: &Regex,
) -> anyhow::Result<Vec<timer::Sample>> {
    let haystack = &*b.haystack;
    timer::run(b, || {
        let mut count = 0;
        for line in haystack.lines() {
            if re.is_match(line) {
                count += 1;
            }
        }
        Ok(count)
    })
}

fn model_grep_captures(
    b: &klv::Benchmark,
    re: &Regex,
) -> anyhow::Result<Vec<timer::Sample>> {
    let haystack = &*b.haystack;
    let mut caps = re.create_captures();
    timer::run(b, || {
        let mut count = 0;
        for line in haystack.lines() {
            let mut input = Input::new(line);
            while let Some(m) = {
                re.search_captures(&input, &mut caps);
                caps.get_match()
            } {
                for i in 0..caps.group_len() {
                    if caps.get_group(i).is_some() {
                        count += 1;
                    }
                }
                // Benchmark definition says we may assume empty matches are
                // impossible.
                input.set_start(m.end());
            }
        }
        Ok(count)
    })
}

fn model_regex_redux(
    b: &klv::Benchmark,
) -> anyhow::Result<Vec<timer::Sample>> {
    let haystack = b.haystack_str()?;
    let compile = |pattern: &str| -> anyhow::Result<regexredux::RegexFn> {
        let re = compile_pattern(b, &[pattern])?;
        let find = move |h: &str| {
            Ok(re.find(h.as_bytes()).map(|m| (m.start(), m.end())))
        };
        Ok(Box::new(find))
    };
    timer::run(b, || regexredux::generic(haystack, compile))
}

fn compile(b: &klv::Benchmark) -> anyhow::Result<Regex> {
    compile_pattern(b, &b.regex.patterns)
}

fn compile_pattern<P: AsRef<str>>(
    b: &klv::Benchmark,
    patterns: &[P],
) -> anyhow::Result<Regex> {
    let config = Regex::config()
        // Disabling UTF-8 here just means that zero-width matches that split
        // a codepoint are allowed.
        .utf8_empty(false)
        .nfa_size_limit(Some((1 << 20) * 100));
    let syntax = regex_automata::util::syntax::Config::new()
        // Disabling UTF-8 just makes it possible to build regexes that won't
        // necessarily match UTF-8. Whether Unicode is actually usable or not
        // depends on the 'unicode' option.
        //
        // This basically corresponds to the use of regex::bytes::Regex instead
        // of regex::Regex.
        .utf8(false)
        .unicode(b.regex.unicode)
        .case_insensitive(b.regex.case_insensitive);

    let re = Regex::builder()
        .configure(config)
        .syntax(syntax)
        .build_many(patterns)?;
    Ok(re)
}
