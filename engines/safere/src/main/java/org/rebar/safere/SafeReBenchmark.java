package org.rebar.safere;

import java.nio.file.Files;
import java.nio.file.Path;
import org.openjdk.jmh.annotations.Benchmark;
import org.openjdk.jmh.annotations.Param;
import org.openjdk.jmh.annotations.Scope;
import org.openjdk.jmh.annotations.Setup;
import org.openjdk.jmh.annotations.State;

/** JMH state that prepares and measures one SafeRE workload in a fork. */
@State(Scope.Thread)
public class SafeReBenchmark {
  /** Path to the Rebar KLV input supplied by the parent runner. */
  @Param("unset")
  public String inputPath;

  /** Input representation selected by the parent runner. */
  @Param("string")
  public String inputMode;

  /** Result that the fork must reproduce before measuring. */
  @Param("0")
  public int expectedCount;

  private Workloads.Workload workload;

  /**
   * Prepares the workload and verifies its result before JMH measures it.
   *
   * @throws Exception if input parsing or workload execution fails
   */
  @Setup
  public void setup() throws Exception {
    InputMode mode = InputMode.parse(inputMode);
    workload =
        Workloads.create(BenchmarkConfig.parse(Files.readAllBytes(Path.of(inputPath)), mode), mode);
    int actual = workload.run();
    if (actual != expectedCount) {
      throw new IllegalStateException(
          "JMH fork count " + actual + " differed from " + expectedCount);
    }
  }

  /**
   * Runs one measured invocation of the prepared workload.
   *
   * @return the workload result consumed by JMH
   * @throws Exception if the workload fails
   */
  @Benchmark
  public int run() throws Exception {
    return workload.run();
  }
}
