use std::{io::Write, ops::ControlFlow};
use {
    anyhow::Context,
    bstr::ByteSlice,
    javelin_pattern::bytes::{Regex, RegexBuilder},
    lexopt::Arg,
};

fn main() -> anyhow::Result<()> {
    let mut parser = lexopt::Parser::from_env();
    let mut backtracking = false;
    let mut quiet = false;
    while let Some(arg) = parser.next()? {
        match arg {
            Arg::Long("version") => {
                println!("{}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            Arg::Long("backtracking") => backtracking = true,
            Arg::Long("quiet") => quiet = true,
            _ => return Err(arg.unexpected().into()),
        }
    }
    let b = klv::Benchmark::read(std::io::stdin())
        .context("failed to read KLV")?;
    let mut builder = RegexBuilder::new();
    builder
        .prefer_backtracking(backtracking)
        .caseless(b.regex.case_insensitive)
        .utf(b.regex.unicode)
        .ucp(b.regex.unicode)
        .unicode_case(b.regex.unicode)
        .auto_cluster(!matches!(
            b.model.as_str(),
            "compile" | "count-captures" | "grep-captures"
        ));
    let compile =
        || builder.build(b.regex.one()?).context("failed to compile pattern");
    let samples = if b.model == "compile" {
        timer::run_and_count(
            &b,
            |re: Regex| Ok(count(&re, &b.haystack)),
            compile,
        )?
    } else if b.model == "regex-redux" {
        let haystack = b.haystack_str()?;
        timer::run(&b, || {
            regexredux::generic(haystack, |pattern| {
                let re = builder.build(pattern)?;
                Ok(Box::new(move |text: &str| {
                    Ok(re.find(text.as_bytes()).map(|m| (m.start(), m.end())))
                }) as regexredux::RegexFn)
            })
        })?
    } else {
        let re = compile()?;
        timer::run(&b, || match b.model.as_str() {
            "count" => Ok(count(&re, &b.haystack)),
            "count-spans" => Ok(count_match_bytes(&re, &b.haystack)),
            "count-captures" => Ok(captures(&re, &b.haystack)),
            "grep" => {
                Ok(b.haystack.lines().filter(|line| re.is_match(line)).count())
            }
            "grep-captures" => {
                Ok(b.haystack.lines().map(|line| captures(&re, line)).sum())
            }
            _ => anyhow::bail!("unrecognized benchmark model {:?}", b.model),
        })?
    };
    if !quiet {
        let mut stdout = std::io::stdout().lock();
        for sample in samples {
            writeln!(
                stdout,
                "{},{}",
                sample.duration.as_nanos(),
                sample.count
            )?;
        }
    }
    Ok(())
}

fn captures(re: &Regex, haystack: &[u8]) -> usize {
    let mut total = 0;
    let _ = re.scan_captures(haystack, |groups| {
        total += groups.iter().flatten().count();
        ControlFlow::<()>::Continue(())
    });
    total
}

fn count(re: &Regex, haystack: &[u8]) -> usize {
    let mut total = 0;
    let _ = re.scan(haystack, |_| {
        total += 1;
        ControlFlow::<()>::Continue(())
    });
    total
}

fn count_match_bytes(re: &Regex, haystack: &[u8]) -> usize {
    let mut total = 0;
    let _ = re.scan(haystack, |matched| {
        total += matched.end() - matched.start();
        ControlFlow::<()>::Continue(())
    });
    total
}
