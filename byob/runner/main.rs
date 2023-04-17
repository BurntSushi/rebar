// WARNING: If you make changes to the code in this file, please update BYOB.md
// in the root of this repository.

use std::{
    io::Write,
    time::{Duration, Instant},
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

macro_rules! err {
    ($($tt:tt)*) => {
        return Err(From::from(format!($($tt)*)))
    }
}

fn main() -> Result<()> {
    let Some(arg) = std::env::args_os().nth(1) else {
        err!("Usage: runner (<engine-name> | --version)")
    };
    let Ok(arg) = arg.into_string() else {
        err!("argument given is not valid UTF-8")
    };
    if arg == "--version" {
        writeln!(std::io::stdout(), env!("CARGO_PKG_VERSION"))?;
        return Ok(());
    }
    let engine = arg;
    let raw = std::io::read_to_string(std::io::stdin())?;
    let config = Config::read(&raw)?;
    let samples = match (&*engine, &*config.model) {
        ("rust/memmem", "iter") => rust_memmem_iter(&config),
        ("rust/memmem/restricted", "iter") => {
            rust_memmem_restricted_iter(&config)
        }
        ("libc/memmem", "iter") => libc_memmem_iter(&config),
        (engine, model) => {
            err!("unrecognized engine '{engine}' and model '{model}'")
        }
    };
    let mut stdout = std::io::stdout().lock();
    for s in samples.iter() {
        writeln!(stdout, "{},{}", s.duration.as_nanos(), s.count)?;
    }
    Ok(())
}

fn rust_memmem_iter(c: &Config) -> Vec<Sample> {
    let finder = memchr::memmem::Finder::new(&c.needle);
    run(c, || {
        let mut haystack = c.haystack.as_bytes();
        let mut count = 0;
        while let Some(i) = finder.find(&haystack) {
            count += 1;
            haystack =
                match haystack.get(i + std::cmp::max(1, c.needle.len())..) {
                    Some(haystack) => haystack,
                    None => break,
                };
        }
        count
    })
}

fn rust_memmem_restricted_iter(c: &Config) -> Vec<Sample> {
    let memmem = memchr::memmem::find;
    run(c, || {
        let mut haystack = c.haystack.as_bytes();
        let mut count = 0;
        while let Some(i) = memmem(&haystack, c.needle.as_bytes()) {
            count += 1;
            haystack =
                match haystack.get(i + std::cmp::max(1, c.needle.len())..) {
                    Some(haystack) => haystack,
                    None => break,
                };
        }
        count
    })
}

fn libc_memmem_iter(c: &Config) -> Vec<Sample> {
    run(c, || {
        let mut haystack = c.haystack.as_bytes();
        let mut count = 0;
        while let Some(i) = libc_memmem(&haystack, c.needle.as_bytes()) {
            count += 1;
            haystack =
                match haystack.get(i + std::cmp::max(1, c.needle.len())..) {
                    Some(haystack) => haystack,
                    None => break,
                };
        }
        count
    })
}

#[derive(Clone, Debug)]
struct Sample {
    duration: Duration,
    count: usize,
}

fn run(c: &Config, mut bench: impl FnMut() -> usize) -> Vec<Sample> {
    let warmup_start = Instant::now();
    for _ in 0..c.max_warmup_iters {
        let _count = bench();
        if warmup_start.elapsed() >= c.max_warmup_time {
            break;
        }
    }

    let mut samples = vec![];
    let run_start = Instant::now();
    for _ in 0..c.max_iters {
        let bench_start = Instant::now();
        let count = bench();
        let duration = bench_start.elapsed();
        samples.push(Sample { duration, count });
        if run_start.elapsed() >= c.max_time {
            break;
        }
    }
    samples
}

#[derive(Clone, Debug, Default)]
struct Config {
    name: String,
    model: String,
    needle: String,
    haystack: String,
    max_iters: u64,
    max_warmup_iters: u64,
    max_time: Duration,
    max_warmup_time: Duration,
}

impl Config {
    fn read(mut raw: &str) -> Result<Config> {
        let mut config = Config::default();
        while !raw.is_empty() {
            let klv = OneKLV::read(raw)?;
            raw = &raw[klv.len..];
            config.set(klv)?;
        }
        Ok(config)
    }

    fn set(&mut self, klv: OneKLV) -> Result<()> {
        let parse_duration = |v: String| -> Result<Duration> {
            Ok(Duration::from_nanos(v.parse()?))
        };
        let OneKLV { key, value, .. } = klv;
        match &*key {
            "name" => self.name = value,
            "model" => self.model = value,
            "pattern" => self.needle = value,
            "haystack" => self.haystack = value,
            "max-iters" => self.max_iters = value.parse()?,
            "max-warmup-iters" => self.max_warmup_iters = value.parse()?,
            "max-time" => self.max_time = parse_duration(value)?,
            "max-warmup-time" => self.max_warmup_time = parse_duration(value)?,
            _ => {}
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
struct OneKLV {
    key: String,
    value: String,
    len: usize,
}

impl OneKLV {
    fn read(mut raw: &str) -> Result<OneKLV> {
        let Some(key_end) = raw.find(':') else {
            err!("invalid KLV item: could not find first ':'")
        };
        let key = &raw[..key_end];
        raw = &raw[key_end + 1..];

        let Some(value_len_end) = raw.find(':') else {
            err!("invalid KLV item: could not find second ':' for '{key}'")
        };
        let value_len_str = &raw[..value_len_end];
        raw = &raw[value_len_end + 1..];

        let Ok(value_len) = value_len_str.parse() else {
            err!(
                "invalid KLV item: value length '{value_len_str}' \
                 is not a number for '{key}'",
            )
        };
        let value = &raw[..value_len];
        if raw.as_bytes()[value_len] != b'\n' {
            err!("invalid KLV item: no line terminator for '{key}'")
        }
        let len = key.len() + 1 + value_len_end + 1 + value.len() + 1;
        Ok(OneKLV { key: key.to_string(), value: value.to_string(), len })
    }
}

/// A safe wrapper around libc's `memmem` function. In particular, this
/// converts memmem's pointer return to an index offset into `haystack`.
fn libc_memmem(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    // SAFETY: We know that both our haystack and needle pointers are valid and
    // non-null, and we also know that the lengths of each corresponds to the
    // number of bytes at that memory region.
    let p = unsafe {
        libc::memmem(
            haystack.as_ptr().cast(),
            haystack.len(),
            needle.as_ptr().cast(),
            needle.len(),
        )
    };
    if p.is_null() {
        None
    } else {
        let start = (p as isize) - (haystack.as_ptr() as isize);
        Some(start as usize)
    }
}
