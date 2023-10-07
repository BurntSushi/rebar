import std.stdio;
import std.regex : matchAll, matchFirst, Regex, regex, RegexMatch, Captures;
import std.datetime.stopwatch : Duration, AutoStart, StopWatch;
import std.algorithm : splitter, map, sum;
import std.array : array;
import std.getopt : getopt, defaultGetoptPrinter;

struct RegexConfig {
    string[] patterns;
    bool case_insensitive;
    bool unicode;
}

struct Benchmark {
    string name;
    string model;
    RegexConfig regex;
    string haystack;
    size_t max_iters;
    size_t max_warmup_iters;
    Duration max_time;
    Duration max_warmup_time;

    void read_from_stdin() {
        import std.conv : to;
        import core.time : dur;

        template parseArg(alias type, string field)
        {
            enum parseArg =
                is(type == string)
                    ? field ~ " = readVal();"
                    : field ~ " = to!" ~ type.stringof ~ "( readVal() );";
        }

        string buf;
        while (!stdin.eof()) {
            buf ~= stdin.readln();
        }

        auto split() {
            import std.algorithm.searching : find;
            auto l = buf.length - find!"a == b"(buf, ':').length;
            auto r = buf[0 .. l];
            if (l >= buf.length) {
                buf = [];
            } else {
                buf = buf[(l+1) .. $];
            }
            return r;
        }

        string readVal() {
            auto sz = to!size_t( to!string( split() ) );
            auto val = to!string( buf[0 .. sz] );
            assert(val.length == sz);
            assert(buf[sz] == '\n');
            buf = buf[sz+1 .. $];
            return val;
        }

        while (buf.length > 0) {
            auto key = split();
            switch (key) {
                case "name":
                    mixin(parseArg!(string, "name"));
                    break;
                case "model":
                    mixin(parseArg!(string, "model"));
                    break;
                case "pattern":
                    this.regex.patterns ~= readVal();
                    break;
                case "case-insensitive":
                    mixin(parseArg!(bool, "regex.case_insensitive"));
                    break;
                case "unicode":
                    mixin(parseArg!(bool, "regex.unicode"));
                    break;
                case "haystack":
                    this.haystack = readVal();
                    break;
                case "max-iters":
                    mixin(parseArg!(size_t, "max_iters"));
                    break;
                case "max-warmup-iters":
                    mixin(parseArg!(size_t, "max_warmup_iters"));
                    break;
                case "max-time":
                    this.max_time = dur!"nsecs"( to!ulong(readVal()) );
                    break;
                case "max-warmup-time":
                    this.max_warmup_time = dur!"nsecs"( to!ulong(readVal()) );
                    break;
                default: {
                    throw new Exception(
                        "unknown key: '" ~ to!string(key) ~ "'"
                    );
                }
            }
        }
    }
}

struct Sample {
    size_t duration;
    size_t count;
}

void run(ref Benchmark b, size_t delegate() bench) {
    run_and_count(b, (ref size_t count) => count, bench);
}

import core.time : MonoTimeImpl, ClockType;

// Forces a MonoTime type with a precise clock
alias MonoTimePrecise = MonoTimeImpl!(ClockType.precise);

// Re-implements a basic std.datetime.stopwatch.StopWatch
// with a more precise timing.
struct PreciseStopWatch {
    void start() @safe nothrow @nogc {
        assert(
            MonoTimePrecise.ticksPerSecond() == 1_000_000_000,
            "Monotonic clock isn't precise enough!"
        );
        _timeStarted = MonoTimePrecise.currTime;
    }

    size_t peek() @safe const nothrow @nogc {
        return MonoTimePrecise.currTime.ticks - _timeStarted.ticks;
    }

private:
    MonoTimePrecise _timeStarted;
}

void run_and_count(T)(
    ref Benchmark b,
    size_t delegate(ref T) count,
    T delegate() bench
) {
    auto warmup_timer = StopWatch(AutoStart.yes);
    for (int i = 0; i < b.max_warmup_iters; i++) {
        auto result = bench();
        auto _count = count(result);
        if (warmup_timer.peek() >= b.max_warmup_time) {
            break;
        }
    }

    Sample[] samples;
    samples.reserve(b.max_iters);

    auto run_timer = StopWatch(AutoStart.yes);
    for (int i = 0; i < b.max_iters; i++) {
        auto bench_timer = PreciseStopWatch();
        bench_timer.start();
        auto result = bench();
        auto duration = bench_timer.peek();
        auto _count = count(result);
        samples ~= Sample( duration, _count );
        if (run_timer.peek() >= b.max_time) {
            break;
        }
    }

    foreach (Sample s; samples) {
        writeln(s.duration, ",", s.count);
    }
}

void main(string[] args) {
    bool showVersion = false;

    auto helpInfo = getopt(
        args,
        "version", &showVersion
    );
    if (helpInfo.helpWanted) {
        defaultGetoptPrinter("Usage:", helpInfo.options);
        return;
    }
    if (showVersion) {
        import std.compiler : version_major, version_minor;
        writeln(version_major, ".", version_minor);
        return;
    }

    Benchmark b;
    b.read_from_stdin();
    switch (b.model) {
        case "compile":         model_compile(b); break;
        case "count":           model_count(b, compile(b)); break;
        case "count-spans":     model_count_spans(b, compile(b)); break;
        case "count-captures":  model_count_captures(b, compile(b)); break;
        case "grep":            model_grep(b, compile(b)); break;
        case "grep-captures":   model_grep_captures(b, compile(b)); break;
        case "regex-redux":     model_regex_redux(b); break;
        default:
            throw new Exception(
                "unrecognized benchmark model '" ~ b.model ~ "'"
            );
    }
}

void model_compile(ref Benchmark b) {
    auto haystack = b.haystack;
    run_and_count(b,
        (ref Regex!char re) => haystack.matchAll(re).array.length,
        () => compile(b));
}

void model_count(ref Benchmark b, Regex!char re) {
    auto haystack = b.haystack;
    run(b, () => haystack.matchAll(re).array.length);
}

void model_count_spans(ref Benchmark b, Regex!char re) {
    auto haystack = b.haystack;
    run(b,
        () => haystack.matchAll(re)
                // count should be the length of the entire match
                .map!((Captures!string m) => m[0].length)
                .sum()
    );
}

void model_count_captures(ref Benchmark b, Regex!char re) {
    auto haystack = b.haystack;
    run(b,
        () => haystack.matchAll(re)
                .map!((Captures!string m) {
                    // need to check here and subtract one since
                    // captures includes the whole match in it's length.
                    if (m.length <= 0) { return 0; }
                    else { return m.length - 1; }
                })
                .sum()
    );
}

void model_grep(ref Benchmark b, Regex!char re) {
    import std.string : lineSplitter;
    auto haystack = b.haystack;
    run(b,
        () {
            size_t count;
            foreach (line; haystack.lineSplitter) {
                if (line.matchFirst(re)) {
                    count += 1;
                }
            }
            return count;
        }
    );
}

void model_grep_captures(ref Benchmark b, Regex!char re) {
    import std.string : lineSplitter;
    auto haystack = b.haystack;
    run(b,
        () {
            size_t count;
            foreach (line; haystack.lineSplitter) {
                foreach (m; line.matchAll(re)) {
                    if (m.length > 0) {
                        count += (m.length - 1);
                    }
                }
            }
            return count;
        }
    );
}

void model_regex_redux(ref Benchmark b) {
    import std.regex : replaceAll;
    import std.conv : to;
    auto haystack = b.haystack;
    run(b,
        () {
            string result;
            auto seq = haystack;
            auto ilen = seq.length;

            auto flatten = regex(">[^\n]*\n|\n");
            seq = seq.replaceAll(flatten, "");
            auto clen = seq.length;

            const string[] variants = [
                "agggtaaa|tttaccct",
                "[cgt]gggtaaa|tttaccc[acg]",
                "a[act]ggtaaa|tttacc[agt]t",
                "ag[act]gtaaa|tttac[agt]ct",
                "agg[act]taaa|ttta[agt]cct",
                "aggg[acg]aaa|ttt[cgt]ccct",
                "agggt[cgt]aa|tt[acg]accct",
                "agggta[cgt]a|t[acg]taccct",
                "agggtaa[cgt]|[acg]ttaccct",
            ];

            static size_t count(Range)(Regex!char re, Range haystack) {
                size_t count = 0;
                foreach (m; matchAll(haystack, re)) {
                    count++;
                }
                return count;
            }

            foreach (variant; variants) {
                auto re = regex(variant);
                result ~= variant;
                result ~= " ";
                result ~= to!string( count(re, seq) );
                result ~= "\n";
            }

            seq = seq.replaceAll(regex("tHa[Nt]"), "<4>");
            seq = seq.replaceAll(regex("aND|caN|Ha[DS]|WaS"), "<3>");
            seq = seq.replaceAll(regex("a[NSt]|BY"), "<2>");
            seq = seq.replaceAll(regex("<[^>]*>"), "|");
            seq = seq.replaceAll(regex("\\|[^|][^|]*\\|"), "-");

            result ~= "\n";
            result ~= to!string(ilen); result ~= "\n";
            result ~= to!string(clen); result ~= "\n";
            result ~= to!string(seq.length);

            static void verify(string output) {
                static const string expected = (
"agggtaaa|tttaccct 6
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
547899");
                assert(expected == output);
            }

            verify(result);

            return seq.length;
        }
    );
}

auto compile(ref Benchmark b) {
    return compile_pattern(b, b.regex.patterns);
}

auto compile_pattern(ref Benchmark b, ref string[] patterns) {
    assert(patterns.length == 1);
    return regex(patterns[0], b.regex.case_insensitive ? "i" : "");
}
