use std::time::{Duration, Instant};

/// A sample computed from a single benchmark iteration.
#[derive(Clone, Debug)]
pub struct Sample {
    /// The duration of the iteration.
    pub duration: Duration,
    /// The count reported by the benchmark. This is used by the harness to
    /// verify that the result is correct.
    ///
    /// All benchmark models except for regex-redux use this. For regex-redux,
    /// it is always zero.
    pub count: u64,
}

/// Run the given `bench` function repeatedly until either the maximum
/// time or number of iterations has been reached and return the set of
/// samples.
pub fn run(
    b: &klv::Benchmark,
    bench: impl FnMut() -> anyhow::Result<usize>,
) -> anyhow::Result<Vec<Sample>> {
    run_and_count(b, |count| Ok(count), bench)
}

/// Run the given `bench` function repeatedly until either the maximum
/// time or number of iterations has been reached and return the set of
/// samples. The count for each sample is determined by running `count` on
/// the result of `bench`. The execution time of `count` is specifically
/// not included in the sample's duration.
///
/// N.B. This variant only exists for the 'compile' model. We want to only
/// measure compile time, but still do extra work that we specifically
/// don't measure to produce a count to ensure the compile regex behaves as
/// expected.
pub fn run_and_count<T>(
    b: &klv::Benchmark,
    mut count: impl FnMut(T) -> anyhow::Result<usize>,
    mut bench: impl FnMut() -> anyhow::Result<T>,
) -> anyhow::Result<Vec<Sample>> {
    let warmup_start = Instant::now();
    for _ in 0..b.max_warmup_iters {
        let result = bench();
        // We still compute the count in case there was a problem doing so,
        // even though we don't do anything with the count.
        let _count = count(result?)?;
        if warmup_start.elapsed() >= b.max_warmup_time {
            break;
        }
    }

    let mut samples = vec![];
    let run_start = Instant::now();
    for _ in 0..b.max_iters {
        let bench_start = Instant::now();
        let result = bench();
        let duration = bench_start.elapsed();
        // Should be fine since it's unreasonable for a match count to
        // exceed u64::MAX.
        let count = u64::try_from(count(result?)?).unwrap();
        samples.push(Sample { duration, count });
        if run_start.elapsed() >= b.max_time {
            break;
        }
    }
    Ok(samples)
}
