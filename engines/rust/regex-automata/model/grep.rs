use bstr::ByteSlice;

use crate::{new, Config};

pub(crate) fn run(c: &Config) -> anyhow::Result<Vec<timer::Sample>> {
    match &*c.engine {
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

fn meta(c: &Config) -> anyhow::Result<Vec<timer::Sample>> {
    let haystack = &*c.b.haystack;
    let re = new::meta(c)?;
    timer::run(&c.b, || {
        let mut count = 0;
        for line in haystack.lines() {
            if re.is_match(line) {
                count += 1;
            }
        }
        Ok(count)
    })
}

fn dense(c: &Config) -> anyhow::Result<Vec<timer::Sample>> {
    let haystack = &*c.b.haystack;
    let re = new::dense(c)?;
    timer::run(&c.b, || {
        let mut count = 0;
        for line in haystack.lines() {
            if re.is_match(line) {
                count += 1;
            }
        }
        Ok(count)
    })
}

fn sparse(c: &Config) -> anyhow::Result<Vec<timer::Sample>> {
    let haystack = &*c.b.haystack;
    let re = new::sparse(c)?;
    timer::run(&c.b, || {
        let mut count = 0;
        for line in haystack.lines() {
            if re.is_match(line) {
                count += 1;
            }
        }
        Ok(count)
    })
}

fn hybrid(c: &Config) -> anyhow::Result<Vec<timer::Sample>> {
    let haystack = &*c.b.haystack;
    let re = new::hybrid(c)?;
    let mut cache = re.create_cache();
    timer::run(&c.b, || {
        let mut count = 0;
        for line in haystack.lines() {
            if re.is_match(&mut cache, line) {
                count += 1;
            }
        }
        Ok(count)
    })
}

fn backtrack(c: &Config) -> anyhow::Result<Vec<timer::Sample>> {
    let haystack = &*c.b.haystack;
    let re = new::backtrack(c)?;
    let mut cache = re.create_cache();
    timer::run(&c.b, || {
        let mut count = 0;
        for line in haystack.lines() {
            if re.try_is_match(&mut cache, line)? {
                count += 1;
            }
        }
        Ok(count)
    })
}

fn pikevm(c: &Config) -> anyhow::Result<Vec<timer::Sample>> {
    let haystack = &*c.b.haystack;
    let re = new::pikevm(c)?;
    let mut cache = re.create_cache();
    timer::run(&c.b, || {
        let mut count = 0;
        for line in haystack.lines() {
            if re.is_match(&mut cache, line) {
                count += 1;
            }
        }
        Ok(count)
    })
}

fn onepass(c: &Config) -> anyhow::Result<Vec<timer::Sample>> {
    let haystack = &*c.b.haystack;
    let re = new::onepass(c)?;
    let mut cache = re.create_cache();
    timer::run(&c.b, || {
        let mut count = 0;
        for line in haystack.lines() {
            if re.is_match(&mut cache, line) {
                count += 1;
            }
        }
        Ok(count)
    })
}
