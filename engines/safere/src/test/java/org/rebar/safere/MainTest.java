package org.rebar.safere;

import static org.junit.jupiter.api.Assertions.assertEquals;

import java.io.ByteArrayOutputStream;
import java.io.PrintStream;
import java.nio.charset.StandardCharsets;
import java.util.List;
import org.junit.jupiter.api.Test;
import org.openjdk.jmh.util.MultisetStatistics;

final class MainTest {
  @Test
  void limitsSamplesAcrossTheFullDurationDistribution() throws Exception {
    assertEquals(List.of("10,7", "20,7", "100,7"), samples(3));
    assertEquals(List.of("20,7"), samples(1));
  }

  @Test
  void preservesEverySampleWhenBelowTheLimit() throws Exception {
    assertEquals(List.of("10,7", "10,7", "20,7", "20,7", "100,7", "100,7"), samples(10));
  }

  private static List<String> samples(long maxIters) throws Exception {
    MultisetStatistics statistics = new MultisetStatistics();
    statistics.addValue(10, 2);
    statistics.addValue(20, 2);
    statistics.addValue(100, 2);
    ByteArrayOutputStream bytes = new ByteArrayOutputStream();
    try (PrintStream output = new PrintStream(bytes, false, StandardCharsets.UTF_8)) {
      Main.emitSamples(statistics, maxIters, 7, output);
    }
    return bytes.toString(StandardCharsets.UTF_8).lines().toList();
  }
}
