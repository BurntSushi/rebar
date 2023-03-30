use std::io::Write;

use {anyhow::Context, lexopt::Arg};

use crate::ffi::{Options, Regex};

mod ffi;

fn main() -> anyhow::Result<()> {
    let mut p = lexopt::Parser::from_env();
    let mut version = false;
    while let Some(arg) = p.next()? {
        match arg {
            Arg::Short('h') | Arg::Long("help") => {
                anyhow::bail!("main [--version]")
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
        "count" => model_count(&b, &mut compile(&b)?)?,
        "count-spans" => model_count_spans(&b, &mut compile(&b)?)?,
        "count-captures" => model_count_captures(&b, &mut compile(&b)?)?,
        "grep" => model_grep(&b, &mut compile(&b)?)?,
        "grep-captures" => model_grep_captures(&b, &mut compile(&b)?)?,
        // N.B. We don't support regex-redux because I didn't feel
        // like implementing a UTF-16 version of it in Rust.
        _ => anyhow::bail!("unrecognized benchmark model '{}'", b.model),
    };
    let mut stdout = std::io::stdout().lock();
    for s in samples.iter() {
        writeln!(stdout, "{},{}", s.duration.as_nanos(), s.count)?;
    }
    Ok(())
}

fn model_compile(b: &klv::Benchmark) -> anyhow::Result<Vec<timer::Sample>> {
    let haystack = utf16(&b.haystack).context("invalid haystack")?;
    timer::run_and_count(
        b,
        |mut re: Regex| re.matcher(&haystack)?.count(),
        || compile(b),
    )
}

fn model_count(
    b: &klv::Benchmark,
    re: &mut Regex,
) -> anyhow::Result<Vec<timer::Sample>> {
    let haystack = utf16(&b.haystack).context("invalid haystack")?;
    timer::run(b, || re.matcher(&haystack)?.count())
}

fn model_count_spans(
    b: &klv::Benchmark,
    re: &mut Regex,
) -> anyhow::Result<Vec<timer::Sample>> {
    let haystack = utf16(&b.haystack).context("invalid haystack")?;
    timer::run(b, || {
        let mut sum = 0;
        let mut m = re.matcher(&haystack)?;
        while m.find()? {
            sum += m.end(0)?.unwrap() - m.start(0)?.unwrap();
        }
        Ok(sum)
    })
}

fn model_count_captures(
    b: &klv::Benchmark,
    re: &mut Regex,
) -> anyhow::Result<Vec<timer::Sample>> {
    let haystack = utf16(&b.haystack).context("invalid haystack")?;
    let group_len = re.group_len()?;
    timer::run(b, || {
        let mut count = 0;
        let mut m = re.matcher(&haystack)?;
        while m.find()? {
            for i in 0..group_len {
                if m.start(i)?.is_some() {
                    count += 1;
                }
            }
        }
        Ok(count)
    })
}

fn model_grep(
    b: &klv::Benchmark,
    re: &mut Regex,
) -> anyhow::Result<Vec<timer::Sample>> {
    let haystack = utf16(&b.haystack).context("invalid haystack")?;
    timer::run(b, || {
        let mut count = 0;
        for line in Lines::new(&haystack) {
            if re.matcher(line)?.find()? {
                count += 1;
            }
        }
        Ok(count)
    })
}

fn model_grep_captures(
    b: &klv::Benchmark,
    re: &mut Regex,
) -> anyhow::Result<Vec<timer::Sample>> {
    let haystack = utf16(&b.haystack).context("invalid haystack")?;
    let group_len = re.group_len()?;
    timer::run(b, || {
        let mut count = 0;
        for line in Lines::new(&haystack) {
            let mut m = re.matcher(line)?;
            while m.find()? {
                for i in 0..group_len {
                    if m.start(i)?.is_some() {
                        count += 1;
                    }
                }
            }
        }
        Ok(count)
    })
}

/// Compile the pattern in the given benchmark configuration to an ICU regex.
fn compile(b: &klv::Benchmark) -> anyhow::Result<Regex> {
    Regex::new(&utf16(b.regex.one()?)?, options(b))
}

/// Build ICU regex options from benchmark configuration.
fn options(b: &klv::Benchmark) -> Options {
    Options { case_insensitive: b.regex.case_insensitive }
}

/// Converts the given bytes to UTF-16. If the bytes aren't valid UTF-8, then
/// an error is returned.
///
/// This works even `&str` too, although it's a little wasteful since it will
/// perform an unnecessary UTF-8 validation. But since we use this routine on
/// small inputs outside of measurements, it's fine.
fn utf16<T: AsRef<[u8]>>(bytes: T) -> anyhow::Result<Vec<u16>> {
    let s =
        String::from_utf8(bytes.as_ref().to_vec()).context("invalid UTF-8")?;
    Ok(s.encode_utf16().collect())
}

struct Lines<'a> {
    haystack: &'a [u16],
}

impl<'a> Lines<'a> {
    fn new(haystack: &'a [u16]) -> Lines<'a> {
        Lines { haystack }
    }
}

impl<'a> Iterator for Lines<'a> {
    type Item = &'a [u16];

    #[inline]
    fn next(&mut self) -> Option<&'a [u16]> {
        let newline = u16::from(b'\n');
        let mut line = match self.haystack.iter().position(|&c| c == newline) {
            None if self.haystack.is_empty() => return None,
            None => {
                let line = self.haystack;
                self.haystack = &[];
                line
            }
            Some(end) => {
                let line = &self.haystack[..end];
                self.haystack = &self.haystack[end + 1..];
                line
            }
        };
        if line.last() == Some(&u16::from(b'\r')) {
            line = &line[..line.len() - 1];
        }
        Some(line)
    }
}
