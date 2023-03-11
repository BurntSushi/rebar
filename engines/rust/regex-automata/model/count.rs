use crate::{new, Config};

pub(crate) fn run(c: &Config) -> anyhow::Result<Vec<timer::Sample>> {
    match &*c.engine {
        "meta" => meta(c),
        "dense" => dense(c),
        "sparse" => sparse(c),
        "hybrid" => hybrid(c),
        "backtrack" => backtrack(c),
        "pikevm" => pikevm(c),
        _ => unreachable!(),
    }
}

fn meta(c: &Config) -> anyhow::Result<Vec<timer::Sample>> {
    let haystack = &*c.b.haystack;
    let re = new::meta(c)?;
    timer::run(&c.b, || Ok(re.find_iter(haystack).count()))
}

fn dense(c: &Config) -> anyhow::Result<Vec<timer::Sample>> {
    let haystack = &*c.b.haystack;
    let re = new::dense(c)?;
    timer::run(&c.b, || Ok(re.find_iter(haystack).count()))
}

fn sparse(c: &Config) -> anyhow::Result<Vec<timer::Sample>> {
    let haystack = &*c.b.haystack;
    let re = new::sparse(c)?;
    timer::run(&c.b, || Ok(re.find_iter(haystack).count()))
}

fn hybrid(c: &Config) -> anyhow::Result<Vec<timer::Sample>> {
    let haystack = &*c.b.haystack;
    let re = new::hybrid(c)?;
    let mut cache = re.create_cache();
    timer::run(&c.b, || Ok(re.find_iter(&mut cache, haystack).count()))
}

fn backtrack(c: &Config) -> anyhow::Result<Vec<timer::Sample>> {
    let haystack = &*c.b.haystack;
    let re = new::backtrack(c)?;
    let mut cache = re.create_cache();
    timer::run(&c.b, || {
        // We could check the haystack length against
        // 'backtrack::min_visited_capacity' and return an error before running
        // our benchmark, but handling the error at search time is probably
        // more consistent with real world usage. Some brief experiments don't
        // seem to show much of a difference between this and the panicking
        // APIs.
        let mut count = 0;
        for result in re.try_find_iter(&mut cache, haystack) {
            result?;
            count += 1;
        }
        Ok(count)
    })
}

fn pikevm(c: &Config) -> anyhow::Result<Vec<timer::Sample>> {
    let haystack = &*c.b.haystack;
    let re = new::pikevm(c)?;
    let mut cache = re.create_cache();
    timer::run(&c.b, || Ok(re.find_iter(&mut cache, haystack).count()))
}
