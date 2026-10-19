package org.rebar.safere;

/** Creates a Rebar workload for the selected SafeRE input mode. */
final class Workloads {
  private Workloads() {}

  static Workload create(BenchmarkConfig config, InputMode mode) {
    return mode == InputMode.STRING
        ? StringWorkloads.create(config)
        : Utf8Workloads.create(config);
  }

  /** One invocation of a Rebar workload that reports its result. */
  @FunctionalInterface
  interface Workload {
    /** Runs the workload and returns the result checked by Rebar. */
    int run() throws Exception;
  }
}
