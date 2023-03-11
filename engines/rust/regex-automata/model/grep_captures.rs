use bstr::ByteSlice;

use regex_automata::{Anchored, Input};

use crate::{new, Config};

pub(crate) fn run(c: &Config) -> anyhow::Result<Vec<timer::Sample>> {
    match &*c.engine {
        "meta" => meta(c),
        "backtrack" => backtrack(c),
        "pikevm" => pikevm(c),
        "onepass" => onepass(c),
        _ => unreachable!(),
    }
}

fn meta(c: &Config) -> anyhow::Result<Vec<timer::Sample>> {
    let haystack = &*c.b.haystack;
    let re = new::meta(c)?;
    let mut caps = re.create_captures();
    timer::run(&c.b, || {
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

fn backtrack(c: &Config) -> anyhow::Result<Vec<timer::Sample>> {
    let haystack = &*c.b.haystack;
    let re = new::backtrack(c)?;
    let (mut cache, mut caps) = (re.create_cache(), re.create_captures());
    timer::run(&c.b, || {
        let mut count = 0;
        for line in haystack.lines() {
            let mut input = Input::new(line);
            while let Some(m) = {
                re.try_search(&mut cache, &input, &mut caps)?;
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

fn pikevm(c: &Config) -> anyhow::Result<Vec<timer::Sample>> {
    let haystack = &*c.b.haystack;
    let re = new::pikevm(c)?;
    let (mut cache, mut caps) = (re.create_cache(), re.create_captures());
    timer::run(&c.b, || {
        let mut count = 0;
        for line in haystack.lines() {
            let mut input = Input::new(line);
            while let Some(m) = {
                re.search(&mut cache, &input, &mut caps);
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

fn onepass(c: &Config) -> anyhow::Result<Vec<timer::Sample>> {
    let haystack = &*c.b.haystack;
    let re = new::onepass(c)?;
    let (mut cache, mut caps) = (re.create_cache(), re.create_captures());
    timer::run(&c.b, || {
        let mut count = 0;
        for line in haystack.lines() {
            let mut input = Input::new(line).anchored(Anchored::Yes);
            while let Some(m) = {
                re.try_search(&mut cache, &input, &mut caps)?;
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
