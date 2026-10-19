package org.rebar.safere;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotSame;
import static org.junit.jupiter.api.Assertions.assertThrows;

import java.io.ByteArrayOutputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import org.safere.Pattern;
import org.safere.Utf8Input;

final class SafeReBenchmarkTest {
  @TempDir Path directory;

  @Test
  void compilationProducesFreshPatternsWithTheConfiguredFlags() throws Exception {
    for (InputMode mode : InputMode.values()) {
      SafeReBenchmark benchmark = compilation(mode);
      benchmark.setup();

      Pattern first = benchmark.compile();
      Pattern second = benchmark.compile();
      assertNotSame(first, second);
      assertEquals(
          Pattern.CASE_INSENSITIVE | Pattern.UNICODE_CASE | Pattern.UNICODE_CHARACTER_CLASS,
          first.flags());
      assertEquals(
          2,
          mode == InputMode.UTF8
              ? Utf8Workloads.countMatches(
                  first, Utf8Input.validated("É é".getBytes(StandardCharsets.UTF_8)))
              : StringWorkloads.countMatches(first, "É é"));
    }
  }

  @Test
  void compilationSetupRejectsAnIncorrectMatchCount() throws Exception {
    for (InputMode mode : InputMode.values()) {
      SafeReBenchmark benchmark = compilation(mode);
      benchmark.expectedCount = 1;
      IllegalStateException error = assertThrows(IllegalStateException.class, benchmark::setup);
      assertEquals("JMH fork count 2 differed from 1", error.getMessage());
    }
  }

  private SafeReBenchmark compilation(InputMode mode) throws Exception {
    ByteArrayOutputStream input = new ByteArrayOutputStream();
    write(input, "model", "compile");
    write(input, "pattern", "é");
    write(input, "haystack", "É é");
    write(input, "case-insensitive", "true");
    write(input, "unicode", "true");
    Path path = directory.resolve(mode.argument() + ".klv");
    Files.write(path, input.toByteArray());

    SafeReBenchmark benchmark = new SafeReBenchmark();
    benchmark.inputPath = path.toString();
    benchmark.inputMode = mode.argument();
    benchmark.expectedCount = 2;
    return benchmark;
  }

  private static void write(ByteArrayOutputStream output, String key, String value) {
    byte[] bytes = value.getBytes(StandardCharsets.UTF_8);
    output.writeBytes((key + ":" + bytes.length + ":").getBytes(StandardCharsets.UTF_8));
    output.writeBytes(bytes);
    output.write('\n');
  }
}
