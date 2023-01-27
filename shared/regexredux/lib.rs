use std::fmt::Write;

/// A closure that implements a regex search. The closure should look for a
/// match in the given haystack, and if one is found, return `Ok(Some(start,
/// end))`. Otherwise, if there's no match, then `Ok(None)` should be returned.
///
/// If the regex engine could not complete the search, then an error should be
/// returned.
pub type RegexFn =
    Box<dyn FnMut(&str) -> anyhow::Result<Option<(usize, usize)>>>;

/// Run the regex-redux benchmark on the given haystack with the given closure.
/// The closure should accept a regex pattern string and compile it to another
/// closure that implements a regex search for that pattern.
///
/// Any errors that occur while compiling a pattern or running a regex search
/// are returned. On success, this returns the length, in bytes, of the
/// transformed input after all replacements have been made.
pub fn generic(
    haystack: &str,
    mut compile: impl FnMut(&str) -> anyhow::Result<RegexFn>,
) -> anyhow::Result<usize> {
    let mut out = String::new();
    let mut seq = haystack.to_string();
    let ilen = seq.len();

    let flatten = compile(r">[^\n]*\n|\n")?;
    seq = replace_all(&seq, "", flatten)?;
    let clen = seq.len();

    let variants = vec![
        r"agggtaaa|tttaccct",
        r"[cgt]gggtaaa|tttaccc[acg]",
        r"a[act]ggtaaa|tttacc[agt]t",
        r"ag[act]gtaaa|tttac[agt]ct",
        r"agg[act]taaa|ttta[agt]cct",
        r"aggg[acg]aaa|ttt[cgt]ccct",
        r"agggt[cgt]aa|tt[acg]accct",
        r"agggta[cgt]a|t[acg]taccct",
        r"agggtaa[cgt]|[acg]ttaccct",
    ];
    for variant in variants {
        let re = compile(variant)?;
        writeln!(out, "{} {}", variant, count(&seq, re)?)?;
    }

    let substs = vec![
        (compile(r"tHa[Nt]")?, "<4>"),
        (compile(r"aND|caN|Ha[DS]|WaS")?, "<3>"),
        (compile(r"a[NSt]|BY")?, "<2>"),
        (compile(r"<[^>]*>")?, "|"),
        (compile(r"\|[^|][^|]*\|")?, "-"),
    ];
    for (re, replacement) in substs.into_iter() {
        seq = replace_all(&seq, replacement, re)?;
    }
    writeln!(out, "\n{}\n{}\n{}", ilen, clen, seq.len())?;
    verify(out)?;
    Ok(seq.len())
}

fn count(
    mut haystack: &str,
    mut find: impl FnMut(&str) -> anyhow::Result<Option<(usize, usize)>>,
) -> anyhow::Result<usize> {
    let mut count = 0;
    // This type of iteration only works in cases where there isn't any
    // look-around and there aren't any empty matches. Which is the case
    // for this benchmark.
    while let Some((_, end)) = find(haystack)? {
        haystack = &haystack[end..];
        count += 1;
    }
    Ok(count)
}

fn replace_all(
    mut haystack: &str,
    replacement: &str,
    mut find: impl FnMut(&str) -> anyhow::Result<Option<(usize, usize)>>,
) -> anyhow::Result<String> {
    let mut new = String::with_capacity(haystack.len());
    // This type of iteration only works in cases where there isn't any
    // look-around and there aren't any empty matches. Which is the case
    // for this benchmark.
    while let Some((start, end)) = find(haystack)? {
        new.push_str(&haystack[..start]);
        new.push_str(replacement);
        haystack = &haystack[end..];
    }
    new.push_str(haystack);
    Ok(new)
}

/// Usually we rely on the rebar harness to verify the results of a benchmark,
/// and that itself just uses a single integer count. But regex-redux wants to
/// check a bit more than a simple count, so we do the verification here. (We
/// do also return the length of the final transformed string too and that is
/// checked by `rebar`, but that length doesn't quite reflect all of the work
/// required by the benchmark model, so we also do this verification step.)
fn verify(output: String) -> anyhow::Result<()> {
    let expected = "\
agggtaaa|tttaccct 6
[cgt]gggtaaa|tttaccc[acg] 26
a[act]ggtaaa|tttacc[agt]t 86
ag[act]gtaaa|tttac[agt]ct 58
agg[act]taaa|ttta[agt]cct 113
aggg[acg]aaa|ttt[cgt]ccct 31
agggt[cgt]aa|tt[acg]accct 31
agggta[cgt]a|t[acg]taccct 32
agggtaa[cgt]|[acg]ttaccct 43

1016745
1000000
547899
";
    anyhow::ensure!(
        expected == &*output,
        "output did not match what was expected",
    );
    Ok(())
}
