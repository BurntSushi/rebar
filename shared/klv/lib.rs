use std::{
    cmp::min,
    io::{Read, Write},
    sync::Arc,
    time::Duration,
};

use {anyhow::Context, bstr::ByteSlice};

/// A single benchmark execution.
///
/// This type knows how to be read from KLV format and written to KLV format.
#[derive(Clone, Debug)]
pub struct Benchmark {
    pub name: String,
    pub model: String,
    pub regex: Regex,
    pub haystack: Arc<[u8]>,
    pub max_iters: u64,
    pub max_warmup_iters: u64,
    pub max_time: Duration,
    pub max_warmup_time: Duration,
}

impl Benchmark {
    /// Parses the entire contents of the given reader as a single benchmark
    /// configuration. The format expect is "KLV," and if there was a problem
    /// parsing the format, then an error is returned.
    ///
    /// Note that a reader that returns zero bytes represents a valid
    /// KLV format containing zero items. (In such a case, the benchmark
    /// configuration will have an empty name and model, which is almost
    /// certainly going to lead to an error when determining how to actually
    /// execute the benchmark.
    pub fn read<R: Read>(mut rdr: R) -> anyhow::Result<Benchmark> {
        // We just slurp everything into memory. While haystacks can sometimes
        // get a little big, it's almost never more than a few MB. We can spare
        // the memory in exchange for simplicity. Besides, the rebar benchmark
        // runner already holds all haystacks in memory anyway.
        let mut buf = vec![];
        rdr.read_to_end(&mut buf)
            .with_context(|| format!("failed to read KLV data into memory"))?;

        let mut bench = Benchmark {
            name: String::default(),
            model: String::default(),
            regex: Regex::default(),
            haystack: Arc::from(vec![]),
            max_iters: u64::default(),
            max_warmup_iters: u64::default(),
            max_time: Duration::default(),
            max_warmup_time: Duration::default(),
        };
        let mut buf = buf.as_slice();
        while !buf.is_empty() {
            let (klv, nread) = OneKLV::read(buf)?;
            buf = &buf[nread..];
            match klv.key.as_str() {
                "name" => {
                    bench.name = klv.to_str()?.to_string();
                }
                "model" => {
                    bench.model = klv.to_str()?.to_string();
                }
                "pattern" => {
                    bench.regex.patterns.push(klv.to_str()?.to_string());
                }
                "case-insensitive" => {
                    bench.regex.case_insensitive = klv.to_bool()?;
                }
                "unicode" => {
                    bench.regex.unicode = klv.to_bool()?;
                }
                "haystack" => {
                    bench.haystack = klv.value;
                }
                "max-iters" => {
                    bench.max_iters = klv.to_u64()?;
                }
                "max-warmup-iters" => {
                    bench.max_warmup_iters = klv.to_u64()?;
                }
                "max-time" => {
                    bench.max_time = klv.to_duration()?;
                }
                "max-warmup-time" => {
                    bench.max_warmup_time = klv.to_duration()?;
                }
                _ => anyhow::bail!("unrecognized KLV key '{}'", klv.key),
            }
        }
        Ok(bench)
    }

    /// Write this benchmark configuration to the given writer in KLV format.
    /// Any errors returned by the given writer are returned to the caller.
    pub fn write<W: Write>(&self, wtr: W) -> anyhow::Result<()> {
        fn imp<W: Write>(b: &Benchmark, mut wtr: W) -> anyhow::Result<()> {
            OneKLV::new("name", &b.name)
                .write(&mut wtr)
                .context("failed to write 'name'")?;

            OneKLV::new("model", &b.model)
                .write(&mut wtr)
                .context("failed to write 'model'")?;

            OneKLV::new(
                "case-insensitive",
                &b.regex.case_insensitive.to_string(),
            )
            .write(&mut wtr)
            .context("failed to write 'case-insensitive'")?;

            OneKLV::new("unicode", &b.regex.unicode.to_string())
                .write(&mut wtr)
                .context("failed to write 'unicode'")?;

            OneKLV::new("max-iters", &b.max_iters.to_string())
                .write(&mut wtr)
                .context("failed to write 'max-iters'")?;

            OneKLV::new("max-warmup-iters", &b.max_warmup_iters.to_string())
                .write(&mut wtr)
                .context("failed to write 'max-warmup-iters'")?;

            OneKLV::new("max-time", &b.max_time.as_nanos().to_string())
                .write(&mut wtr)
                .context("failed to write 'max-time'")?;

            OneKLV::new(
                "max-warmup-time",
                &b.max_warmup_time.as_nanos().to_string(),
            )
            .write(&mut wtr)
            .context("failed to write 'max-warmup-time'")?;

            // We write the patterns and haystack last because they can be big.
            // If there are things after it, they can be easy to miss. This is
            // also why we write patterns second to last, since there can be
            // many patterns. (But usually there's only one.)
            for (i, p) in b.regex.patterns.iter().enumerate() {
                OneKLV::new("pattern", p).write(&mut wtr).with_context(
                    || format!("failed to write pattern {}", i),
                )?;
            }
            OneKLV {
                key: "haystack".to_string(),
                value: Arc::clone(&b.haystack),
            }
            .write(&mut wtr)
            .context("failed to write 'haystack'")?;

            Ok(())
        }
        imp(self, wtr).with_context(|| {
            format!("failed to write benchmark '{}' in KLV format", self.name)
        })
    }

    /// Return the haystack in this benchmark as a UTF-8 encoded string. This
    /// will return an error if the haystack is invalid UTF-8.
    ///
    /// Most benchmarks use a haystack that is valid UTF-8, but some do not.
    /// Some regex engines (like 'regress', at time of writing) do not provide
    /// APIs for running the regex engine on invalid UTF-8. Thus, benchmarks
    /// for those regex engines should use this method to ensure the haystack
    /// is valid UTF-8. Generally speaking, this means those engines should not
    /// be run at all for benchmarks using invalid UTF-8 in their haystacks.
    pub fn haystack_str(&self) -> anyhow::Result<&str> {
        self.haystack.to_str().context("failed to decode haystack as UTF-8")
    }
}

// We do this manually because Arc<[u8]> doesn't have a Default impl...
impl Default for Benchmark {
    fn default() -> Benchmark {
        Benchmark {
            name: String::default(),
            model: String::default(),
            regex: Regex::default(),
            haystack: Arc::from(vec![]),
            max_iters: u64::default(),
            max_warmup_iters: u64::default(),
            max_time: Duration::default(),
            max_warmup_time: Duration::default(),
        }
    }
}

/// The configuration of zero or more regex patterns in a single benchmark.
#[derive(Clone, Debug, Default)]
pub struct Regex {
    /// The patterns that should be compiled to a regular expression.
    ///
    /// Zero patterns is legal, although more regex engines expected exactly
    /// one. In which case, that regex engine should report an error.
    pub patterns: Vec<String>,
    /// Whether the patterns should be compiled case insensitively.
    pub case_insensitive: bool,
    /// Whether the patterns should be compiled with Unicode mode enabled.
    ///
    /// Unicode mode means somewhat different things to different regex
    /// engines, but generally it's expected that it results in matching
    /// codepoints as the atomic unit instead of individual bytes. It also
    /// usually enables the use of things like \pL and makes things like .,
    /// [^a] and \w Unicode aware.
    pub unicode: bool,
}

impl Regex {
    /// When the configuration contains exactly one pattern, then return that
    /// pattern. Otherwise, including when the number of patterns is zero,
    /// return an error.
    ///
    /// This is useful to get a single pattern for regex engines that only
    /// support compiling one pattern.
    pub fn one(&self) -> anyhow::Result<&str> {
        anyhow::ensure!(
            self.patterns.len() == 1,
            "regex engine only supports one pattern at a time, \
             but was given {} patterns",
            self.patterns.len(),
        );
        Ok(&self.patterns[0])
    }
}

/// Represents a single key-length-value pair. It knows how to read and write
/// them and returns user-friendly error messages.
#[derive(Clone)]
struct OneKLV {
    key: String,
    value: Arc<[u8]>,
}

impl OneKLV {
    /// A convenience constructor for creating one new KLV item from UTF-8
    /// encoded strings.
    fn new(key: &str, value: &str) -> OneKLV {
        OneKLV { key: key.to_string(), value: Arc::from(value.as_bytes()) }
    }

    /// Read a single KLV starting at the beginning of the given slice of
    /// bytes. The slice given may contain more than a single KLV. Upon
    /// success, the second element of the tuple returned corresponds to the
    /// total number of bytes read from the slice.
    fn read(bytes: &[u8]) -> anyhow::Result<(OneKLV, usize)> {
        let mut nread = 0;
        let (key, bytes) = match bytes.split_once_str(":") {
            Some(x) => x,
            None => anyhow::bail!(
                "failed to find first ':' in key-length-value item \
                 where the next (at most) 80 bytes are: {:?}",
                bytes[..min(80, bytes.len())].as_bstr(),
            ),
        };
        nread += key.len() + 1; // +1 for ':'
        let key = key
            .to_str()
            .with_context(|| {
                format!("key {:?} is not valid UTF-8", key.as_bstr())
            })?
            .to_string();

        let (len, bytes) = match bytes.split_once_str(":") {
            Some(x) => x,
            None => anyhow::bail!(
                "failed to find second ':' in key-length-value item \
                 for key '{}'",
                key,
            ),
        };
        nread += len.len() + 1; // +1 for ':'
        let len = len.to_str().with_context(|| {
            format!("length for key '{}' is not valid UTF-8", key)
        })?;
        let len = len.parse::<usize>().with_context(|| {
            format!(
                "length '{}' for key '{}' is not a valid integer",
                len, key,
            )
        })?;

        anyhow::ensure!(
            bytes.len() >= len,
            "got length of {} for key '{}', but only {} bytes remain",
            len,
            key,
            bytes.len(),
        );
        let value = bytes[..len].into();
        let bytes = &bytes[len..];
        nread += len;

        anyhow::ensure!(
            bytes.len() >= 1,
            "expected trailing '\\n' after value, but got EOF",
        );
        anyhow::ensure!(
            bytes[0] == b'\n',
            "expected '\\n' after value, but got {:?}",
            bytes[0..1].as_bstr(),
        );
        nread += 1;

        let klv = OneKLV { key, value };
        Ok((klv, nread))
    }

    /// Writes this single KLV to the buffer given.
    ///
    /// This panics if the key contains a ':'.
    fn write<W: Write>(&self, mut wtr: W) -> anyhow::Result<()> {
        assert!(
            !self.key.contains(':'),
            "keys must not contain ':' but '{}' does",
            self.key,
        );
        let len = self.value.len().to_string();
        wtr.write_all(self.key.as_bytes())?;
        wtr.write_all(b":")?;
        wtr.write_all(len.as_bytes())?;
        wtr.write_all(b":")?;
        wtr.write_all(&self.value)?;
        wtr.write_all(b"\n")?;
        Ok(())
    }

    /// Return the value in this KLV as UTF-8, or an error if the value is
    /// invalid UTF-8.
    fn to_str(&self) -> anyhow::Result<&str> {
        self.value.to_str().with_context(|| {
            format!("expected valid UTF-8 for value for key '{}'", self.key)
        })
    }

    /// Parse the value as a boolean 'true' or 'false' value, otherwise return
    /// an error.
    fn to_bool(&self) -> anyhow::Result<bool> {
        self.to_str()?.parse().with_context(|| {
            format!("expected boolean value for key '{}'", self.key)
        })
    }

    /// Parse the value as a u64 integer, otherwise return an error.
    fn to_u64(&self) -> anyhow::Result<u64> {
        self.to_str()?.parse().with_context(|| {
            format!(
                "expected unsigned 64-bit integer value for key '{}'",
                self.key,
            )
        })
    }

    /// Parse the value as a duration in nanoseconds, otherwise return an
    /// error.
    fn to_duration(&self) -> anyhow::Result<Duration> {
        self.to_str()?.parse().map(Duration::from_nanos).with_context(|| {
            format!(
                "expected nanoseconds integer value for key '{}'",
                self.key,
            )
        })
    }
}

impl std::fmt::Debug for OneKLV {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("OneKLV")
            .field("key", &self.key)
            .field("value", &self.value.as_bstr())
            .finish()
    }
}
