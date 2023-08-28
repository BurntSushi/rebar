use std::time::Duration;

use {
    anyhow::Context,
    bstr::{BString, ByteSlice},
};

/// The rebar Cargo package version. This environment variable is guaranteed
/// to be made available by Cargo.
pub const REBAR_VERSION: &'static str = env!("CARGO_PKG_VERSION");

/// The commit revision hash that rebar was built from. This environment
/// variable is set by a custom build script, and is only available when `git`
/// is available.
pub const REBAR_REVISION: Option<&'static str> = option_env!("REBAR_REVISION");

/// Returns a complete version string for `rebar`.
///
/// If `git` was available while building `rebar`, then this includes the
/// revision hash.
pub fn version() -> String {
    let mut s = REBAR_VERSION.to_string();
    if let Some(rev) = REBAR_REVISION {
        s.push_str(&format!(" (rev {})", rev));
    }
    s
}

/// A simple little wrapper type around std::time::Duration that permits
/// serializing and deserializing using a basic human friendly short duration.
///
/// We can get away with being simple here by assuming the duration is short.
/// i.e., No longer than one minute. So all we handle here are seconds,
/// milliseconds, microseconds and nanoseconds.
///
/// This avoids bringing in another crate to do this work (like humantime).
/// Hah, incidentally, when I wrote this, I had forgotten that I already had
/// an indirect dependency on 'humantime' via 'env_logger'. I decided to keep
/// this type because it lets us precisely control the format we support.
#[derive(Clone, Copy, Default)]
pub struct ShortHumanDuration(Duration);

impl ShortHumanDuration {
    pub fn serialize_with<S: serde::Serializer>(
        d: &Duration,
        s: S,
    ) -> Result<S::Ok, S::Error> {
        serde::Serialize::serialize(&ShortHumanDuration::from(*d), s)
    }

    pub fn deserialize_with<'de, D: serde::Deserializer<'de>>(
        d: D,
    ) -> Result<Duration, D::Error> {
        let sdur: ShortHumanDuration = serde::Deserialize::deserialize(d)?;
        Ok(Duration::from(sdur))
    }
}

impl From<ShortHumanDuration> for Duration {
    fn from(hdur: ShortHumanDuration) -> Duration {
        hdur.0
    }
}

impl From<Duration> for ShortHumanDuration {
    fn from(dur: Duration) -> ShortHumanDuration {
        ShortHumanDuration(dur)
    }
}

impl std::fmt::Debug for ShortHumanDuration {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::Display::fmt(self, f)
    }
}

impl std::fmt::Display for ShortHumanDuration {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let v = self.0.as_secs_f64();
        if v >= 0.950 {
            write!(f, "{:.2}s", v)
        } else if v >= 0.000_950 {
            write!(f, "{:.2}ms", v * 1_000.0)
        } else if v >= 0.000_000_950 {
            write!(f, "{:.2}us", v * 1_000_000.0)
        } else {
            write!(f, "{:.2}ns", v * 1_000_000_000.0)
        }
    }
}

impl std::str::FromStr for ShortHumanDuration {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> anyhow::Result<ShortHumanDuration> {
        let re = regex!(
            r"(?x)
                ^
                (?P<float>[0-9]+(?:\.[0-9]*)?|\.[0-9]+)
                (?P<units>s|ms|us|ns)
                $
            ",
        );
        // Special case: if we have 0, then it's the same regardless of units.
        if s == "0" {
            return Ok(ShortHumanDuration::default());
        }
        let caps = match re.captures(s) {
            Some(caps) => caps,
            None => anyhow::bail!(
                "duration '{}' not in '<decimal>(s|ms|us|ns)' format",
                s,
            ),
        };
        let mut value: f64 =
            caps["float"].parse().context("invalid duration decimal")?;
        match &caps["units"] {
            "s" => value /= 1.0,
            "ms" => value /= 1_000.0,
            "us" => value /= 1_000_000.0,
            "ns" => value /= 1_000_000_000.0,
            unit => unreachable!("impossible unit '{}'", unit),
        }
        Ok(ShortHumanDuration(Duration::from_secs_f64(value)))
    }
}

impl serde::Serialize for ShortHumanDuration {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> serde::Deserialize<'de> for ShortHumanDuration {
    fn deserialize<D>(deserializer: D) -> Result<ShortHumanDuration, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct V;

        impl<'de> serde::de::Visitor<'de> for V {
            type Value = ShortHumanDuration;

            fn expecting(
                &self,
                f: &mut std::fmt::Formatter,
            ) -> std::fmt::Result {
                write!(f, "duration string of the form <decimal>(s|ms|us|ns)")
            }

            fn visit_str<E>(self, s: &str) -> Result<ShortHumanDuration, E>
            where
                E: serde::de::Error,
            {
                s.parse::<ShortHumanDuration>()
                    .map_err(|e| serde::de::Error::custom(e.to_string()))
            }
        }
        deserializer.deserialize_str(V)
    }
}

/// Another little wrapper type for computing, serializing and deserializing
/// throughput.
///
/// We fix our time units for throughput to "per second," but try to show
/// convenient size units, e.g., GB, MB, KB or B.
///
/// The internal representation is always in bytes per second.
#[derive(Clone, Copy, PartialEq, PartialOrd, Default)]
pub struct Throughput(f64);

impl Throughput {
    /// Create a new throughput from the given number of bytes and the amount
    /// of time taken to process those bytes.
    pub fn new(bytes: u64, duration: Duration) -> Throughput {
        let bytes_per_second = (bytes as f64) / duration.as_secs_f64();
        Throughput::from_bytes_per_second(bytes_per_second)
    }

    /// If you've already computed a throughput and it is in units of B/sec,
    /// then this permits building a `Throughput` from that raw value.
    pub fn from_bytes_per_second(bytes_per_second: f64) -> Throughput {
        Throughput(bytes_per_second)
    }
}

impl std::fmt::Debug for Throughput {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::Display::fmt(self, f)
    }
}

impl std::fmt::Display for Throughput {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        const KB: f64 = (1 << 10) as f64;
        const MB: f64 = (1 << 20) as f64;
        const GB: f64 = (1 << 30) as f64;
        const MIN_KB: f64 = 2.0 * KB;
        const MIN_MB: f64 = 2.0 * MB;
        const MIN_GB: f64 = 2.0 * GB;

        let bytes_per_second = self.0 as f64;
        if bytes_per_second < MIN_KB {
            write!(f, "{} B/s", bytes_per_second as u64)
        } else if bytes_per_second < MIN_MB {
            write!(f, "{:.1} KB/s", bytes_per_second / KB)
        } else if bytes_per_second < MIN_GB {
            write!(f, "{:.1} MB/s", bytes_per_second / MB)
        } else {
            write!(f, "{:.1} GB/s", bytes_per_second / GB)
        }
    }
}

impl std::str::FromStr for Throughput {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> anyhow::Result<Throughput> {
        let re = regex!(
            r"(?x)
                ^
                (?P<float>[0-9]+(?:\.[0-9]*)?|\.[0-9]+)
                \s*
                (?P<units>B|KB|MB|GB)/s
                $
            ",
        );
        let caps = match re.captures(s) {
            Some(caps) => caps,
            None => anyhow::bail!(
                "throughput '{}' not in '<decimal>(B|KB|MB|GB)/s' format",
                s,
            ),
        };
        let mut bytes_per_second: f64 = caps["float"]
            .parse()
            .context("invalid throughput decimal number")?;
        match &caps["units"] {
            "B" => bytes_per_second *= (1 << 0) as f64,
            "KB" => bytes_per_second *= (1 << 10) as f64,
            "MB" => bytes_per_second *= (1 << 20) as f64,
            "GB" => bytes_per_second *= (1 << 30) as f64,
            unit => unreachable!("impossible unit '{}'", unit),
        }
        Ok(Throughput(bytes_per_second))
    }
}

impl serde::Serialize for Throughput {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> serde::Deserialize<'de> for Throughput {
    fn deserialize<D>(deserializer: D) -> Result<Throughput, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct V;

        impl<'de> serde::de::Visitor<'de> for V {
            type Value = Throughput;

            fn expecting(
                &self,
                f: &mut std::fmt::Formatter,
            ) -> std::fmt::Result {
                write!(
                    f,
                    "throughput string of the form <decimal>(B|KB|MB|GB)/s"
                )
            }

            fn visit_str<E>(self, s: &str) -> Result<Throughput, E>
            where
                E: serde::de::Error,
            {
                s.parse::<Throughput>()
                    .map_err(|e| serde::de::Error::custom(e.to_string()))
            }
        }
        deserializer.deserialize_str(V)
    }
}

/// Returns the current executable path as a UTF-8 encoded string, but with a
/// good contextualized error message if it fails.
pub fn current_exe() -> anyhow::Result<String> {
    // I suppose this could fail if 'rebar' is inside a directory where *any*
    // path component is invalid UTF-8. Seems... unlikely, but if you wind up
    // here because of that, then please file an issue.
    std::env::current_exe()
        .context("could not get current executable path")?
        .into_os_string()
        .into_string()
        .map_err(|_| anyhow::anyhow!("current executable path is not UTF-8"))
}

/// Write the given divider character `width` times to the given writer.
pub fn write_divider<W: std::io::Write>(
    mut wtr: W,
    divider: char,
    width: usize,
) -> anyhow::Result<()> {
    let div: String = std::iter::repeat(divider).take(width).collect();
    write!(wtr, "{}", div)?;
    Ok(())
}

/// Colorize the given writer in a "label" style.
pub fn colorize_label<W: termcolor::WriteColor>(
    mut wtr: W,
    mut with: impl FnMut(&mut W) -> std::io::Result<()>,
) -> anyhow::Result<()> {
    let mut spec = termcolor::ColorSpec::new();
    spec.set_bold(true);
    wtr.set_color(&spec)?;
    with(&mut wtr)?;
    wtr.reset()?;
    Ok(())
}

/// Colorize the given writer in a "error" style.
pub fn colorize_error<W: termcolor::WriteColor>(
    mut wtr: W,
    mut with: impl FnMut(&mut W) -> std::io::Result<()>,
) -> anyhow::Result<()> {
    let mut spec = termcolor::ColorSpec::new();
    spec.set_fg(Some(termcolor::Color::Red));
    spec.set_bold(true);
    wtr.set_color(&spec)?;
    with(&mut wtr)?;
    wtr.reset()?;
    Ok(())
}

/// Colorize the given writer in a "note" style.
pub fn colorize_note<W: termcolor::WriteColor>(
    mut wtr: W,
    mut with: impl FnMut(&mut W) -> std::io::Result<()>,
) -> anyhow::Result<()> {
    let mut spec = termcolor::ColorSpec::new();
    spec.set_fg(Some(termcolor::Color::Blue));
    spec.set_bold(true);
    wtr.set_color(&spec)?;
    with(&mut wtr)?;
    wtr.reset()?;
    Ok(())
}

/// This runs the given command synchronously. If there was a problem running
/// the command, then stderr is inspected and its last line is used to
/// construct the error message returned. (The entire stderr is logged at debug
/// level however.)
pub fn output(cmd: &mut std::process::Command) -> anyhow::Result<BString> {
    log::debug!("running command: {:?}", cmd);
    let out =
        cmd.output().context("failed to run command and wait for output")?;
    if out.status.success() {
        if !out.stderr.is_empty() {
            log::debug!(
                "success, but stderr is not empty: {}",
                out.stderr.as_bstr()
            );
        }
        return Ok(BString::from(out.stdout));
    }
    log::debug!("command failed, exit status: {:?}", out.status);
    log::debug!("stderr: {}", out.stderr.as_bstr());
    anyhow::ensure!(
        !out.stderr.is_empty(),
        "command failed with {:?} but stderr is empty",
        out.status,
    );
    let last = match out.stderr.lines().last() {
        Some(last) => last,
        None => {
            anyhow::bail!(
                "command failed with {:?} but stderr is empty",
                out.status,
            )
        }
    };
    Err(anyhow::anyhow!(
        "command failed, last line of stderr: {:?}",
        last.as_bstr(),
    ))
}
