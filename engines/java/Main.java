import java.io.ByteArrayOutputStream;
import java.nio.ByteBuffer;
import java.nio.charset.CharsetDecoder;
import java.nio.charset.StandardCharsets;
import java.nio.charset.CodingErrorAction;
import java.util.ArrayList;
import java.util.List;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

final class Config {
    public String name;
    public String model;
    public String pattern;
    public boolean caseInsensitive;
    public boolean unicode;
    // rebar benchmarks permit the haystack to be invalid UTF-8,
    // but Java's regex engine can only search sequences of UTF-16
    // code units (as far as I can tell). So there's no point in
    // trying to represent the haystack as a byte[]. If this runner
    // program is called with a haystack that contains invalid UTF-8,
    // then it will throw an exception.
    public String haystack;
    public int maxIters;
    public int maxWarmupIters;
    public long maxTime;
    public long maxWarmupTime;

    public Pattern CompileRegex() {
        return CompilePattern(this.pattern);
    }

    public Pattern CompilePattern(String pat) {
        int flags = 0;
        if (this.caseInsensitive) {
            flags |= Pattern.CASE_INSENSITIVE;
        }
        if (this.unicode) {
            flags |= Pattern.UNICODE_CASE;
            flags |= Pattern.UNICODE_CHARACTER_CLASS;
        }
        return Pattern.compile(pat, flags);
    }
}

// A single Key-Length-Value item.
final class OneKLV {
    // The key name.
    public String key;
    // The value contents.
    public String value;
    // The length, in bytes, used up by this KLV item.
    // This is useful for parsing a sequence of KLV items.
    // This length says how much to skip ahead to start
    // parsing the next KLV item.
    public int length;

    public OneKLV(CharsetDecoder decoder, List<Byte> raw) throws Exception {
        int keyEnd = raw.indexOf((byte)':');
        if (keyEnd == -1) {
            throw new Exception("invalid KLV item: could not find first ':'");
        }
        this.key = decode(decoder, raw.subList(0, keyEnd));
        this.length = keyEnd + 1;
        raw = raw.subList(keyEnd + 1, raw.size());

        int valueLenEnd = raw.indexOf((byte)':');
        if (valueLenEnd == -1) {
            throw new Exception("invalid KLV item: could not find second ':'");
        }
        String valueLenStr = decode(decoder, raw.subList(0, valueLenEnd));
        this.length += valueLenEnd + 1;
        raw = raw.subList(valueLenEnd + 1, raw.size());

        int valueLen = Integer.parseInt(valueLenStr);
        if (raw.get(valueLen) != (byte)'\n') {
            throw new Exception("invalid KLV item: no line terminator");
        }
        this.length += valueLen + 1; // +1 for the line terminator
        this.value = decode(decoder, raw.subList(0, valueLen));
    }

    // Decodes a list of Bytes into a string.
    static String decode(
        CharsetDecoder decoder,
        List<Byte> list
    ) throws Exception {
        return decoder.decode(toByteBuffer(list)).toString();
    }

    // Convert a list of bytes to a ByteBuffer. This is so we can decode the
    // raw UTF-8 bytes. What a kludge.
    static ByteBuffer toByteBuffer(List<Byte> list) {
        byte[] bytes = new byte[list.size()];
        for(int i = 0; i < list.size(); i++){
            bytes[i] = list.get(i);
        }
        return ByteBuffer.wrap(bytes);

    }
}

// A representation of the data we gather from a single
// benchmark execution. That is, the time it took to run
// and the count reported for verification.
final class Sample {
    // The duration, in nanoseconds. This might not always
    // have nanosecond resolution, but its units are always
    // nanoseconds.
    public long duration;
    // The count reported by the benchmark. This is checked
    // against what is expected in the benchmark definition
    // by rebar.
    public int count;

    public Sample(long duration, int count) {
        this.duration = duration;
        this.count = count;
    }
}

public final class Main {
    public static void main(String... args) throws Exception {
        if (args.length == 1 && args[0].equals("version")) {
            String vmname = System.getProperty("java.vm.name");
            String vmversion = System.getProperty("java.vm.version");
            System.out.printf("%s %s\n", vmname, vmversion);
            System.exit(0);
        }

        // We create a decoder that will specifically fail on invalid UTF-8.
        // This prevents cases where we get an invalid UTF-8 haystack and
        // silently lossily decode it. Then you're in a situation where the
        // Java regex engine will run on a different haystack than other regex
        // engines. Instead, we loudly fail, which simply means that the Java
        // regex engine can't be used with haystacks that contain invalid
        // UTF-8.
        CharsetDecoder decoder = StandardCharsets.UTF_8.newDecoder()
            .onMalformedInput(CodingErrorAction.REPORT)
            .onUnmappableCharacter(CodingErrorAction.REPORT);

        List<Byte> raw = readStdin();
        Config config = new Config();
        while (raw.size() > 0) {
            OneKLV klv = new OneKLV(decoder, raw);
            raw = raw.subList(klv.length, raw.size());
            if (klv.key.equals("name")) {
                config.name = klv.value;
            } else if (klv.key.equals("model")) {
                config.model = klv.value;
            } else if (klv.key.equals("pattern")) {
                config.pattern = klv.value;
            } else if (klv.key.equals("case-insensitive")) {
                config.caseInsensitive = klv.value.equals("true");
            } else if (klv.key.equals("unicode")) {
                config.unicode = klv.value.equals("true");
            } else if (klv.key.equals("haystack")) {
                config.haystack = klv.value;
            } else if (klv.key.equals("max-iters")) {
                config.maxIters = Integer.parseInt(klv.value);
            } else if (klv.key.equals("max-warmup-iters")) {
                config.maxWarmupIters = Integer.parseInt(klv.value);
            } else if (klv.key.equals("max-time")) {
                config.maxTime = Long.parseLong(klv.value);
            } else if (klv.key.equals("max-warmup-time")) {
                config.maxWarmupTime = Long.parseLong(klv.value);
            } else {
                throw new Exception(String.format(
                    "unrecognized KLV key '%s'",
                    klv.key
                ));
            }
        }
        if (!config.model.equals("regex-redux") && config.pattern == null) {
            throw new Exception("missing pattern, must be provided once");
        }

        // Run our selected model and print the samples.
        List<Sample> samples;
        if (config.model.equals("compile")) {
            samples = ModelCompile(config);
        } else if (config.model.equals("count")) {
            samples = ModelCount(config);
        } else if (config.model.equals("count-spans")) {
            samples = ModelCountSpans(config);
        } else if (config.model.equals("count-captures")) {
            samples = ModelCountCaptures(config);
        } else if (config.model.equals("grep")) {
            samples = ModelGrep(config);
        } else if (config.model.equals("grep-captures")) {
            samples = ModelGrepCaptures(config);
        } else if (config.model.equals("regex-redux")) {
            samples = ModelRegexRedux(config);
        } else {
            throw new Exception(String.format(
                "unrecognized benchmark model %s",
                config.model
            ));
        }
        for (int i = 0; i < samples.size(); i++) {
            Sample s = samples.get(i);
            System.out.printf("%d,%d\n", s.duration, s.count);
        }
    }

    static List<Sample> ModelCompile(Config config) throws Exception {
        return RunAndCount(
            config,
            re -> {
                int count = 0;
                Matcher m = re.matcher(config.haystack);
                while (m.find()) {
                    count++;
                }
                return count;
            },
            () -> config.CompileRegex()
        );
    }

    static List<Sample> ModelCount(Config config) throws Exception {
        Pattern re = config.CompileRegex();
        return RunAndCount(
            config,
            n -> n,
            () -> {
                int count = 0;
                Matcher m = re.matcher(config.haystack);
                while (m.find()) {
                    count++;
                }
                return count;
            }
        );
    }

    static List<Sample> ModelCountSpans(Config config) throws Exception {
        Pattern re = config.CompileRegex();
        return RunAndCount(
            config,
            n -> n,
            () -> {
                int sum = 0;
                Matcher m = re.matcher(config.haystack);
                while (m.find()) {
                    sum += m.end() - m.start();
                }
                return sum;
            }
        );
    }

    static List<Sample> ModelCountCaptures(Config config) throws Exception {
        Pattern re = config.CompileRegex();
        return RunAndCount(
            config,
            n -> n,
            () -> {
                int count = 0;
                Matcher m = re.matcher(config.haystack);
                while (m.find()) {
                    for (int i = 0; i < m.groupCount() + 1; i++) {
                        String cap = m.group(i);
                        if (cap != null) {
                            count++;
                        }
                    }
                }
                return count;
            }
        );
    }

    static List<Sample> ModelGrep(Config config) throws Exception {
        Pattern re = config.CompileRegex();
        return RunAndCount(
            config,
            n -> n,
            () -> {
                // Oh my, Java doesn't support a way to mutate a captured
                // variable directly, so we have to stuff the count inside
                // an array of length 1.
                int[] count = new int[]{0};
                config.haystack.lines().forEach(line -> {
                    if (re.matcher(line).find()) {
                        count[0]++;
                    }
                });
                return count[0];
            }
        );
    }

    static List<Sample> ModelGrepCaptures(Config config) throws Exception {
        Pattern re = config.CompileRegex();
        return RunAndCount(
            config,
            n -> n,
            () -> {
                int[] count = new int[]{0};
                config.haystack.lines().forEach(line -> {
                    Matcher m = re.matcher(line);
                    while (m.find()) {
                        for (int i = 0; i < m.groupCount() + 1; i++) {
                            String cap = m.group(i);
                            if (cap != null) {
                                count[0]++;
                            }
                        }
                    }
                });
                return count[0];
            }
        );
    }

    static List<Sample> ModelRegexRedux(Config config) throws Exception {
        return RunAndCount(
            config,
            n -> n,
            () -> {
                String expected = """
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

                StringBuilder result = new StringBuilder();
                String seq = config.haystack;
                int ilen = seq.length();
                seq = config
                    .CompilePattern(">[^\n]*\n|\n")
                    .matcher(seq)
                    .replaceAll("");
                int clen = seq.length();

                String[] variants = new String[]{
                    "agggtaaa|tttaccct",
                    "[cgt]gggtaaa|tttaccc[acg]",
                    "a[act]ggtaaa|tttacc[agt]t",
                    "ag[act]gtaaa|tttac[agt]ct",
                    "agg[act]taaa|ttta[agt]cct",
                    "aggg[acg]aaa|ttt[cgt]ccct",
                    "agggt[cgt]aa|tt[acg]accct",
                    "agggta[cgt]a|t[acg]taccct",
                    "agggtaa[cgt]|[acg]ttaccct",
                };
                for (int i = 0; i < variants.length; i++) {
                    String variant = variants[i];
                    Pattern re = config.CompilePattern(variant);
                    int count = 0;
                    Matcher m = re.matcher(seq);
                    while (m.find()) {
                        count++;
                    }
                    result.append(String.format("%s %d\n", variant, count));
                }

                seq = config
                    .CompilePattern("tHa[Nt]")
                    .matcher(seq)
                    .replaceAll("<4>");
                seq = config
                    .CompilePattern("aND|caN|Ha[DS]|WaS")
                    .matcher(seq)
                    .replaceAll("<3>");
                seq = config
                    .CompilePattern("a[NSt]|BY")
                    .matcher(seq)
                    .replaceAll("<2>");
                seq = config
                    .CompilePattern("<[^>]*>")
                    .matcher(seq)
                    .replaceAll("|");
                seq = config
                    .CompilePattern("\\|[^|][^|]*\\|")
                    .matcher(seq)
                    .replaceAll("-");

                result.append(String.format(
                    "\n%d\n%d\n%d\n", ilen, clen, seq.length()
                ));
                if (!result.toString().trim().equals(expected.trim())) {
                    throw new Exception("result did not match expected");
                }
                return seq.length();
            }
        );
    }

    // I guess Java does not have anonymous closure types either? (Same as
    // .NET?) I couldn't find it in their sections on delegates or lambda
    // expressions. Oh well.
    interface Count<T> {
        int call(T t) throws Exception;
    }
    interface Bench<T> {
        T call() throws Exception;
    }

    static <T> List<Sample> RunAndCount(
        Config config,
        Count<T> count,
        Bench<T> bench
    ) throws Exception {
        long warmupStart = System.nanoTime();
        for (int i = 0; i < config.maxWarmupIters; i++) {
            T result = bench.call();
            count.call(result);
            if ((System.nanoTime() - warmupStart) >= config.maxWarmupTime) {
                break;
            }
        }

        List<Sample> samples = new ArrayList<>();
        long runStart = System.nanoTime();
        for (int i = 0; i < config.maxIters; i++) {
            long benchStart = System.nanoTime();
            T result = bench.call();
            long elapsed = System.nanoTime() - benchStart;
            int n = count.call(result);
            samples.add(new Sample(elapsed, n));
            if ((System.nanoTime() - runStart) >= config.maxTime) {
                break;
            }
        }
        return samples;
    }

    static List<Byte> readStdin() throws Exception {
        ByteArrayOutputStream out = new ByteArrayOutputStream();
        byte[] buf = new byte[1024];
        int nread;
        while ((nread = System.in.read(buf)) > 0) {
            out.write(buf, 0, nread);
        }
        List<Byte> list = new ArrayList<>();
        for (byte b : out.toByteArray()) {
            list.add(b);
        }
        return list;
    }
}
