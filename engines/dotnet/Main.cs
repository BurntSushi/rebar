using System.Diagnostics;
using System.Text;
using System.Text.RegularExpressions;

/// <summary>The configuration of a benchmark, as parsed from the incoming KLV
/// data.</summary>
struct Config
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

    public Config(string engineName) => engine = engineName;

    public Regex CompileRegex() => CompilePattern(pattern!);

    public Regex CompilePattern(string pat) {
        // We enable the invariant culture to avoid any sort of tailoring.
        // Tailoring might be useful to benchmark, but rebar doesn't do it.
        // Primarily because most (not all) regex engines don't support it.
        RegexOptions options = RegexOptions.CultureInvariant;
        if (caseInsensitive) {
            options |= RegexOptions.IgnoreCase;
        }
        options |= engine switch
        {
            "interp" => RegexOptions.None,
            "compiled" => RegexOptions.Compiled,
            "nobacktrack" => RegexOptions.NonBacktracking,
            _ => throw new Exception($"unrecognized engine '${engine}'"),
        };

        return new Regex(pat, options);
    }
}

/// <summary>A single Key-Length-Value item.</summary>
struct OneKLV
{
    /// <summary>The key name.</summary>
    public string key;
    /// <summary>The value contents.</summary>
    public string value;
    /// <summary>
    /// The length, in bytes, used up by this KLV item.
    /// This is useful for parsing a sequence of KLV items.
    /// This length says how much to skip ahead to start
    /// parsing the next KLV item.
    /// </summary>
    public int len;

    public OneKLV(ReadOnlySpan<byte> raw) {
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
        Encoding utf8 = Encoding.GetEncoding(
            "utf-8",
            new EncoderExceptionFallback(),
            new DecoderExceptionFallback()
        );

        var keyEnd = raw.IndexOf((byte)':');
        if (keyEnd < 0) {
            throw new Exception("invalid KLV item: could not find first ':'");
        }
        key = utf8.GetString(raw.Slice(0, keyEnd));
        raw = raw.Slice(keyEnd + 1);

        var valueLenEnd = raw.IndexOf((byte)':');
        if (valueLenEnd < 0) {
            throw new Exception("invalid KLV item: could not find second ':'");
        }
        int valueLen = int.Parse(utf8.GetString(raw.Slice(0, valueLenEnd)));

        if (raw[valueLenEnd + 1 + valueLen] != '\n') {
            throw new Exception("invalid KLV item: no line terminator");
        }
        value = utf8.GetString(raw.Slice(valueLenEnd + 1, valueLen));
        len = keyEnd + 1 + valueLenEnd + 1 + valueLen + 1;
    }
}

/// <summary>
/// A representation of the data we gather from a single benchmark execution.
/// That is, the time it took to run and the count reported for verification.
/// </summary>
/// <param name="duration">
/// The duration, in nanoseconds. This might not always
/// have nanosecond resolution, but its units are always
/// nanoseconds.
/// </param>
/// <param name="count">
/// The count reported by the benchmark. This is checked
/// against what is expected in the benchmark definition
/// by rebar.
/// </param>
record struct Sample(long duration, int count);

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
            Console.WriteLine(Environment.Version);
            return;
        }

        // Read all of stdin into a span of bytes
        MemoryStream stdinCopy = new();
        using (Stream stdin = Console.OpenStandardInput()) {
            stdin.CopyTo(stdinCopy);
        }
        ReadOnlySpan<byte> raw = stdinCopy.GetBuffer().AsSpan(
            0,
            (int)stdinCopy.Length
        );

        // OK, now read each of our KLV items and build up our config.
        Config config = new(args[0]);
        while (!raw.IsEmpty) {
            var klv = new OneKLV(raw);
            raw = raw.Slice(klv.len);
            switch (klv.key)
            {
                case "name":
                    config.name = klv.value;
                    break;
                case "model":
                    config.model = klv.value;
                    break;
                case "pattern":
                    if (config.pattern != null) {
                        throw new Exception("only one pattern is supported");
                    }
                    config.pattern = klv.value;
                    break;
                case "case-insensitive":
                    config.caseInsensitive = klv.value == "true";
                    break;
                case "unicode":
                    config.unicode = klv.value == "unicode";
                    break;
                case "haystack":
                    config.haystack = klv.value;
                    break;
                case "max-iters":
                    config.maxIters = int.Parse(klv.value);
                    break;
                case "max-warmup-iters":
                    config.maxWarmupIters = int.Parse(klv.value);
                    break;
                case "max-time":
                    config.maxTime = long.Parse(klv.value);
                    break;
                case "max-warmup-time":
                    config.maxWarmupTime = long.Parse(klv.value);
                    break;
                default:
                    throw new Exception($"unrecognized KLV key {klv.key}");
            }
        }

        if (config.model != "regex-redux" && config.pattern == null) {
            throw new Exception("missing pattern, must be provided once");
        }

        // Run our selected model and print the samples.
        List<Sample> samples = config.model switch
        {
            "compile" => ModelCompile(config),
            "count" => ModelCount(config),
            "count-spans" => ModelCountSpans(config),
            "count-captures" => ModelCountCaptures(config),
            "grep" => ModelGrep(config),
            "grep-captures" => ModelGrepCaptures(config),
            "regex-redux" => ModelRegexRedux(config),
            _ => throw new Exception(
                $"unknown benchmark model {config.model}"
            ),
        };

        foreach (Sample s in samples) {
            Console.WriteLine($"{s.duration},{s.count}");
        }
    }

    static List<Sample> ModelCompile(Config config)
    {
        return RunAndCount(
            config,
            re => re.Count(config.haystack!),
            config.CompileRegex
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
                foreach (ValueMatch m in re.EnumerateMatches(config.haystack!)) {
                    // This is not quite the same as most other regex
                    // engines, which report span lengths in terms of
                    // number of bytes. This is in terms of UTF-16 code
                    // units. We deal this by permitting different counts
                    // for .NET regex engines in the benchmark definition.
                    count += m.Length;
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
                Match m = re.Match(config.haystack!);
                while (m.Success) {
                    foreach (Group g in m.Groups) {
                        if (g.Success) {
                            count++;
                        }
                    }
                    m = m.NextMatch();
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
                int count = 0;
                var span = config.haystack.AsSpan();
                foreach (ReadOnlySpan<char> line in span.EnumerateLines()) {
                    if (re.IsMatch(line)) {
                        count++;
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
                int count = 0;
                var span = config.haystack.AsSpan();
                foreach (ReadOnlySpan<char> line in span.EnumerateLines()) {
                    Match m = re.Match(line.ToString());
                    while (m.Success) {
                        foreach (Group g in m.Groups) {
                            if (g.Success) {
                                count++;
                            }
                        }
                        m = m.NextMatch();
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
                var result = new StringBuilder();
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
                    throw new Exception("result did not match expected");
                }
                return seq.Length;
            }
        );
    }

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
        Func<T, int> count,
        Func<T> bench
    )
    {
        Stopwatch warmupTimer = Stopwatch.StartNew();
        for (int i = 0; i < config.maxWarmupIters; i++) {
            var result = bench();
            count(result);
            if (warmupTimer.Elapsed.TotalNanoseconds >= config.maxWarmupTime) {
                break;
            }
        }

        List<Sample> samples = new();
        Stopwatch runTimer = Stopwatch.StartNew();
        for (int i = 0; i < config.maxIters; i++) {
            Stopwatch benchTimer = Stopwatch.StartNew();
            var result = bench();
            var elapsed = benchTimer.Elapsed.TotalNanoseconds;
            var n = count(result);
            samples.Add(new Sample((long)elapsed, n));
            if (runTimer.Elapsed.TotalNanoseconds >= config.maxTime) {
                break;
            }
        }
        return samples;
    }
}
