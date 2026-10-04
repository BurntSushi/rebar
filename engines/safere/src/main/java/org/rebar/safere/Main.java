package org.rebar.safere;

import java.io.InputStream;
import java.io.PrintStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Iterator;
import java.util.List;
import java.util.Map;
import java.util.Properties;
import java.util.concurrent.Callable;
import java.util.concurrent.TimeUnit;
import java.util.function.ToIntFunction;
import org.openjdk.jmh.annotations.Mode;
import org.openjdk.jmh.results.RunResult;
import org.openjdk.jmh.runner.Runner;
import org.openjdk.jmh.runner.options.Options;
import org.openjdk.jmh.runner.options.OptionsBuilder;
import org.openjdk.jmh.runner.options.TimeValue;
import org.openjdk.jmh.util.Statistics;
import org.safere.Pattern;
import org.safere.Utf8Input;

/** Entry point for Rebar's SafeRE engines. */
public final class Main {
  private record Sample(long duration, int count) {}

  private Main() {}

  /**
   * Runs a Rebar workload or prints the SafeRE version.
   *
   * @param args {@code string} or {@code utf8}, optionally followed by {@code version}
   * @throws Exception if input parsing or workload execution fails
   */
  public static void main(String... args) throws Exception {
    if (args.length < 1 || args.length > 2) {
      throw new IllegalArgumentException("usage: Main <string|utf8> [version]");
    }
    InputMode mode = InputMode.parse(args[0]);
    if (args.length == 2 && args[1].equals("version")) {
      printVersion();
      return;
    }
    if (args.length != 1) {
      throw new IllegalArgumentException("usage: Main <string|utf8> [version]");
    }

    byte[] raw = System.in.readAllBytes();
    BenchmarkConfig config = BenchmarkConfig.parse(raw, mode);
    if (config.model().equals("compile")) {
      Utf8Input utf8Input =
          mode == InputMode.UTF8 ? Utf8Input.validated(config.haystackBytes()) : null;
      for (Sample sample :
          sampleDirect(
              config,
              config::compileRegex,
              pattern ->
                  mode == InputMode.UTF8
                      ? Utf8Workloads.countMatches(pattern, utf8Input)
                      : StringWorkloads.countMatches(pattern, config.haystack()))) {
        System.out.printf("%d,%d%n", sample.duration(), sample.count());
      }
      return;
    }

    Workloads.Workload workload = Workloads.create(config, mode);
    // A zero measurement budget still runs one operation for its count.
    if (config.maxTime() == 0) {
      for (Sample sample : sampleDirect(config, workload::run, Integer::intValue)) {
        System.out.printf("%d,%d%n", sample.duration(), sample.count());
      }
      return;
    }
    int count = workload.run();
    runJmh(raw, config, mode, count);
  }

  private static void printVersion() throws Exception {
    Properties properties = new Properties();
    try (InputStream stream =
        Pattern.class.getResourceAsStream("/META-INF/maven/org.safere/safere/pom.properties")) {
      if (stream == null) {
        throw new IllegalStateException("SafeRE version metadata is missing");
      }
      properties.load(stream);
    }
    String version = properties.getProperty("version");
    if (version == null) {
      throw new IllegalStateException("SafeRE version metadata has no version");
    }
    System.out.printf(
        "SafeRE %s (%s %s; scanner=%s)%n",
        version,
        System.getProperty("java.vm.name"),
        System.getProperty("java.vm.version"),
        System.getProperty("org.safere.experimental.vectorScanProvider", "swar"));
  }

  private static void runJmh(byte[] raw, BenchmarkConfig config, InputMode mode, int count)
      throws Exception {
    Path input = Files.createTempFile("rebar-safere-", ".klv");
    Path output = Files.createTempFile("rebar-safere-jmh-", ".log");
    try {
      Files.write(input, raw);
      int warmupIterations = config.maxWarmupIters() > 0 && config.maxWarmupTime() > 0 ? 1 : 0;
      var optionsBuilder =
          new OptionsBuilder()
              .include("^" + SafeReBenchmark.class.getName() + ".run$")
              .mode(Mode.SampleTime)
              .timeUnit(TimeUnit.NANOSECONDS)
              .forks(1)
              .threads(1)
              .warmupIterations(warmupIterations)
              .warmupTime(TimeValue.nanoseconds(config.maxWarmupTime()))
              .measurementIterations(1)
              .measurementTime(TimeValue.nanoseconds(config.maxTime()))
              .param("inputPath", input.toString())
              .param("inputMode", mode.argument())
              .param("expectedCount", Integer.toString(count))
              .output(output.toString());
      if ("vector".equals(System.getProperty("org.safere.experimental.vectorScanProvider"))) {
        // The JMH fork needs the same provider and incubator module as its parent process.
        optionsBuilder.jvmArgsAppend(
            "--add-modules=jdk.incubator.vector",
            "-Dorg.safere.experimental.vectorScanProvider=vector");
      }
      Options options = optionsBuilder.build();
      var results = new Runner(options).run();
      if (results.size() != 1) {
        throw new Exception("JMH returned " + results.size() + " results; expected one");
      }
      RunResult result = results.iterator().next();
      emitSamples(result.getPrimaryResult().getStatistics(), config.maxIters(), count, System.out);
    } catch (Exception e) {
      System.err.print(Files.readString(output));
      throw e;
    } finally {
      Files.deleteIfExists(input);
      Files.deleteIfExists(output);
    }
  }

  static void emitSamples(Statistics statistics, long maxIters, int count, PrintStream output)
      throws Exception {
    long total = statistics.getN();
    long limit = Math.min(total, maxIters);
    if (limit == 0) {
      throw new Exception("JMH returned no samples");
    }
    Iterator<Map.Entry<Double, Long>> samples = statistics.getRawData();
    long seen = 0;
    long emitted = 0;
    while (samples.hasNext()) {
      Map.Entry<Double, Long> sample = samples.next();
      long duration = Math.max(1, Math.round(sample.getKey()));
      seen += sample.getValue();
      // JMH's raw samples are sorted by duration. Select evenly across the
      // cumulative distribution so a small max-iters does not keep only the fastest samples.
      long selected = Math.round((double) seen * limit / total);
      long n = selected - emitted;
      for (long i = 0; i < n; i++) {
        output.printf("%d,%d%n", duration, count);
      }
      emitted = selected;
    }
    if (seen != total || emitted != limit) {
      throw new Exception("JMH sample count changed while reporting results");
    }
  }

  private static <T> List<Sample> sampleDirect(
      BenchmarkConfig config, Callable<T> operation, ToIntFunction<T> count) throws Exception {
    long warmupStart = System.nanoTime();
    for (int i = 0; i < config.maxWarmupIters(); i++) {
      operation.call();
      if ((System.nanoTime() - warmupStart) >= config.maxWarmupTime()) {
        break;
      }
    }

    List<Sample> samples = new ArrayList<>();
    long runStart = System.nanoTime();
    for (int i = 0; i < config.maxIters(); i++) {
      long benchStart = System.nanoTime();
      T result = operation.call();
      long elapsed = System.nanoTime() - benchStart;
      int n = count.applyAsInt(result);
      samples.add(new Sample(elapsed, n));
      if ((System.nanoTime() - runStart) >= config.maxTime()) {
        break;
      }
    }
    return samples;
  }
}
