using System;
using System.IO;
using System.Text.RegularExpressions;
using System.Diagnostics;

// The configuration of a benchmark, as parsed from the incoming KLV data.
public struct Config
{
    public string engine;
    public string? name;
    public string? model;
    public string? pattern;
    public bool caseInsensitive;
    public bool unicode;
    // rebar benchmarks permit the haystack to be invalid UTF-8,
    // but .NET's regex engine can only search sequences of UTF-16
    // code units (as far as I can tell). So there's no point in
    // trying to represent the haystack as a byte[]. If this runner
    // program is called with a haystack that contains invalid UTF-8,
    // then it will throw an exception.
    public string? haystack;
    public int maxIters;
    public int maxWarmupIters;
    public long maxTime;
    public long maxWarmupTime;

    public Config(string engineName) {
        engine = engineName;
        caseInsensitive = false;
        unicode = false;
        maxIters = 0;
        maxWarmupIters = 0;
        maxTime = 0;
        maxWarmupTime = 0;
    }

    public Regex CompileRegex() {
        return CompilePattern(pattern!);
    }

    public Regex CompilePattern(string pat) {
        // We enable the invariant culture to avoid any sort of tailoring.
        // Tailoring might be useful to benchmark, but rebar doesn't do it.
        // Primarily because most (not all) regex engines don't support it.
        RegexOptions options = RegexOptions.CultureInvariant;
        if (caseInsensitive) {
            options |= RegexOptions.IgnoreCase;
        }
        if (engine == "interp") {
            // nothing to do here
        } else if (engine == "compiled") {
            options |= RegexOptions.Compiled;
        } else if (engine == "nobacktrack") {
            options |= RegexOptions.NonBacktracking;
        } else {
            throw new Exception($"unrecognized engine '${engine}'");
        }
        return new Regex(pat, options);
    }
}

// A single Key-Length-Value item.
public struct OneKLV
{
    // The key name.
    public string key;
    // The value contents.
    public string value;
    // The length, in bytes, used up by this KLV item.
    // This is useful for parsing a sequence of KLV items.
    // This length says how much to skip ahead to start
    // parsing the next KLV item.
    public int len;

    public OneKLV(List<byte> raw) {
        // The default for the UTF-8 encoding is to lossily decode bytes.
        // But we really don't want to do that here. We want to return an
        // error, since otherwise, this runner could silently search a slightly
        // different haystack than what other runner programs do.
        //
        // This means that this runner program must only be used in benchmarks
        // with valid UTF-8. This is not an arbitrary decision. As far as I can
        // tell, C#'s regex engine only works on String or ReadOnlySpan<char>,
        // both of which are Unicode strings and incapable of representing
        // arbitrary bytes.
        System.Text.Encoding utf8 = System.Text.Encoding.GetEncoding(
                "utf-8",
                new System.Text.EncoderExceptionFallback(),
                new System.Text.DecoderExceptionFallback()
        );

        var keyEnd = raw.IndexOf((byte)':');
        if (keyEnd == -1) {
            throw new Exception("invalid KLV item: could not find first ':'");
        }
        key = utf8.GetString(raw.GetRange(0, keyEnd).ToArray());

        var valueLenEnd = raw.IndexOf((byte)':', keyEnd + 1);
        if (valueLenEnd == -1) {
            throw new Exception("invalid KLV item: could not find second ':'");
        }
        string valueLenStr = utf8.GetString(
            raw.GetRange(keyEnd + 1, valueLenEnd - (keyEnd + 1)).ToArray()
        );
        int valueLen = int.Parse(valueLenStr);

        if (raw[valueLenEnd + 1 + valueLen] != (byte)'\n') {
            throw new Exception("invalid KLV item: no line terminator");
        }
        value = utf8.GetString(
            raw.GetRange(valueLenEnd + 1, valueLen).ToArray()
        );
        len = valueLenEnd + 1 + valueLen + 1;
    }
}

// A representation of the data we gather from a single
// benchmark execution. That is, the time it took to run
// and the count reported for verification.
public struct Sample
{
    // The duration, in nanoseconds. This might not always
    // have nanosecond resolution, but its units are always
    // nanoseconds.
    public long duration;
    // The count reported by the benchmark. This is checked
    // against what is expected in the benchmark definition
    // by rebar.
    public int count;

    public Sample(long durationNanos, int benchCount)
    {
        duration = durationNanos;
        count = benchCount;
    }
}

class Program
{
    static void Main(string[] args)
    {
        if (args.Length != 1) {
            throw new Exception(
                "Usage: main <interp | compiled | nobacktrack | version>"
            );
        }
        if (args[0] == "version") {
            Console.WriteLine(Environment.Version.ToString());
            return;
        }
        // This is pretty brutal, but 'Console.In' is actually a 'TextReader',
        // and that in turn automatically applies an encoding before returning
        // a Unicode string. But our KLV format really wants to be treated as
        // raw bytes. In particular, the "length" in the key-length-value of
        // each item is the number of UTF-8 encoded bytes in the value field.
        // By the time we get to that point when using 'Console.In', we already
        // have strings and the actual number of bytes we need to account for
        // has been lost.
        //
        // So we do this dance with raw bytes, which is just amazingly
        // inconvenient, because C# doesn't have byte string support.
        Stream stdin = Console.OpenStandardInput();
        List<byte> raw = new List<byte>();
        byte[] buf = new byte[8192];
        int nread = 0;
        while ((nread = stdin.Read(buf, 0, buf.Length)) > 0) {
            for (int i = 0; i < nread; i++) {
                raw.Add(buf[i]);
            }
        }
        // OK, now read each of our KLV items and build up our config.
        Config config = new Config(args[0]);
        while (raw.Count > 0) {
            var klv = new OneKLV(raw);
            raw = raw.GetRange(klv.len, raw.Count - klv.len);
            if (klv.key == "name") {
                config.name = klv.value;
            } else if (klv.key == "model") {
                config.model = klv.value;
            } else if (klv.key == "pattern") {
                if (config.pattern != null) {
                    throw new Exception("only one pattern is supported");
                }
                config.pattern = klv.value;
            } else if (klv.key == "case-insensitive") {
                config.caseInsensitive = klv.value == "true";
            } else if (klv.key == "unicode") {
                config.unicode = klv.value == "unicode";
            } else if (klv.key == "haystack") {
                config.haystack = klv.value;
            } else if (klv.key == "max-iters") {
                config.maxIters = int.Parse(klv.value);
            } else if (klv.key == "max-warmup-iters") {
                config.maxWarmupIters = int.Parse(klv.value);
            } else if (klv.key == "max-time") {
                config.maxTime = long.Parse(klv.value);
            } else if (klv.key == "max-warmup-time") {
                config.maxWarmupTime = long.Parse(klv.value);
            } else {
                throw new Exception($"unrecognized KLV key {klv.key}");
            }
        }
        if (config.model != "regex-redux" && config.pattern == null) {
            throw new Exception("missing pattern, must be provided once");
        }

        // Run our selected model and print the samples.
        List<Sample> samples;
        if (config.model == "compile") {
            samples = ModelCompile(config);
        } else if (config.model == "count") {
            samples = ModelCount(config);
        } else if (config.model == "count-spans") {
            samples = ModelCountSpans(config);
        } else if (config.model == "count-captures") {
            samples = ModelCountCaptures(config);
        } else if (config.model == "grep") {
            samples = ModelGrep(config);
        } else if (config.model == "grep-captures") {
            samples = ModelGrepCaptures(config);
        } else if (config.model == "regex-redux") {
            samples = ModelRegexRedux(config);
        } else {
            throw new Exception($"unknown benchmark model {config.model}");
        }
        foreach (Sample s in samples) {
            Console.WriteLine($"{s.duration},{s.count}");
        }
    }

    static List<Sample> ModelCompile(Config config)
    {
        return RunAndCount(
            config,
            re => re.Count(config.haystack!),
            () => config.CompileRegex()
        );
    }

    static List<Sample> ModelCount(Config config)
    {
        var re = config.CompileRegex();
        return RunAndCount(
            config,
            n => n,
            () => re.Count(config.haystack!)
        );
    }

    static List<Sample> ModelCountSpans(Config config)
    {
        var re = config.CompileRegex();
        return RunAndCount(
            config,
            n => n,
            () => {
                int count = 0;
                foreach (Match m in re.Matches(config.haystack!)) {
                    // This is not quite the same as most other regex
                    // engines, which report span lengths in terms of
                    // number of bytes. This is in terms of UTF-16 code
                    // units. We deal this by permitting different counts
                    // for .NET regex engines in the benchmark definition.
                    count += m.ValueSpan.Length;
                }
                return count;
            }
        );
    }

    static List<Sample> ModelCountCaptures(Config config)
    {
        var re = config.CompileRegex();
        return RunAndCount(
            config,
            n => n,
            () => {
                int count = 0;
                foreach (Match m in re.Matches(config.haystack!)) {
                    foreach (Group g in m.Groups) {
                        if (g.Success) {
                            count += 1;
                        }
                    }
                }
                return count;
            }
        );
    }

    static List<Sample> ModelGrep(Config config)
    {
        var re = config.CompileRegex();
        return RunAndCount(
            config,
            n => n,
            () => {
                StringReader rdr = new StringReader(config.haystack!);
                int count = 0;
                string? line;
                while ((line = rdr.ReadLine()) != null) {
                    if (re.IsMatch(line)) {
                        count += 1;
                    }
                }
                return count;
            }
        );
    }

    static List<Sample> ModelGrepCaptures(Config config)
    {
        var re = config.CompileRegex();
        return RunAndCount(
            config,
            n => n,
            () => {
                StringReader rdr = new StringReader(config.haystack!);
                int count = 0;
                string? line;
                while ((line = rdr.ReadLine()) != null) {
                    foreach (Match m in re.Matches(line)) {
                        foreach (Group g in m.Groups) {
                            if (g.Success) {
                                count += 1;
                            }
                        }
                    }
                }
                return count;
            }
        );
    }

    static List<Sample> ModelRegexRedux(Config config)
    {
        return RunAndCount(
            config,
            n => n,
            () => {
                var expected = """
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

""";
                var result = new System.Text.StringBuilder();
                var seq = config.haystack!;
                var ilen = seq.Length;
                seq = config.CompilePattern(@">[^\n]*\n|\n").Replace(seq, "");
                var clen = seq.Length;

                var variants = new string[] {
                    @"agggtaaa|tttaccct",
                    @"[cgt]gggtaaa|tttaccc[acg]",
                    @"a[act]ggtaaa|tttacc[agt]t",
                    @"ag[act]gtaaa|tttac[agt]ct",
                    @"agg[act]taaa|ttta[agt]cct",
                    @"aggg[acg]aaa|ttt[cgt]ccct",
                    @"agggt[cgt]aa|tt[acg]accct",
                    @"agggta[cgt]a|t[acg]taccct",
                    @"agggtaa[cgt]|[acg]ttaccct",
                };
                foreach (string variant in variants) {
                    var re = config.CompilePattern(variant);
                    var count = re.Count(seq);
                    result.AppendLine($"{variant} {count}");
                }

                seq = config.CompilePattern(@"tHa[Nt]").Replace(seq, "<4>");
                seq = config.CompilePattern(@"aND|caN|Ha[DS]|WaS").Replace(seq, "<3>");
                seq = config.CompilePattern(@"a[NSt]|BY").Replace(seq, "<2>");
                seq = config.CompilePattern(@"<[^>]*>").Replace(seq, "|");
                seq = config.CompilePattern(@"\|[^|][^|]*\|").Replace(seq, "-");

                result.AppendLine("");
                result.AppendLine($"{ilen}");
                result.AppendLine($"{clen}");
                result.AppendLine($"{seq.Length}");
                if (result.ToString() != expected) {
                    Console.WriteLine(result.ToString());
                    Console.WriteLine("===========");
                    Console.WriteLine(expected);
                    throw new Exception("result did not match expected");
                }
                return seq.Length;
            }
        );
    }

    // Does C# really not have anonymous closure types? I couldn't find it in
    // their sections on delegates or lambda expressions. Oh well.
    public delegate int Count<T>(T t);
    public delegate T Bench<T>();

    // Takes in a benchmark config, a closure that returns the count from the
    // benchmark function and a benchmark function that returns a result that
    // can be converted into a count. As output, it returns a list of samples
    // generated by repeatedly running the 'bench' function and timing how long
    // it takes.
    //
    // In practice, 'bench' returns the count and 'count' is just the identity
    // function in all except for one case: measuring compilation time. In
    // that case, 'bench' returns the regex object itself, and 'count' runs the
    // regex to get the count.
    //
    // The 'count' function is not part of the measurement.
    static List<Sample> RunAndCount<T>(
        Config config,
        Count<T> count,
        Bench<T> bench
    )
    {
        Stopwatch warmupTimer = Stopwatch.StartNew();
        for (int i = 0; i < config.maxWarmupIters; i++) {
            var result = bench();
            count(result);
            if (ElapsedNanos(warmupTimer) >= config.maxWarmupTime) {
                break;
            }
        }

        List<Sample> samples = new List<Sample>();
        Stopwatch runTimer = Stopwatch.StartNew();
        for (int i = 0; i < config.maxIters; i++) {
            Stopwatch benchTimer = Stopwatch.StartNew();
            var result = bench();
            var elapsed = ElapsedNanos(benchTimer);
            var n = count(result);
            samples.Add(new Sample(elapsed, n));
            if (ElapsedNanos(runTimer) >= config.maxTime) {
                break;
            }
        }
        return samples;
    }

    // Return the elapsed time on the given stop watch in terms of
    // nano-seconds.
    //
    // Note that .NET doesn't guarantee that the stop-watch uses nanosecond
    // resolution. So this might only be, for example, capable of returning
    // nanoseconds in intervals of 100. But the units returned are indeed
    // always nanoseconds.
    static long ElapsedNanos(Stopwatch sw)
    {
        long nanosPerTick = (1000L * 1000L * 1000L) / Stopwatch.Frequency;
        return nanosPerTick * sw.ElapsedTicks;
    }
}
