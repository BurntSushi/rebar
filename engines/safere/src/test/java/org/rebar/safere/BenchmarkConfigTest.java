package org.rebar.safere;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertThrows;

import java.io.ByteArrayOutputStream;
import java.nio.charset.CharacterCodingException;
import java.nio.charset.StandardCharsets;
import org.junit.jupiter.api.Test;
import org.safere.Pattern;
import org.safere.Utf8Input;

final class BenchmarkConfigTest {
  @Test
  void parsesStringInputAndRegexFlags() throws Exception {
    byte[] haystack = "café".getBytes(StandardCharsets.UTF_8);
    BenchmarkConfig config = BenchmarkConfig.parse(validKlv(haystack), InputMode.STRING);

    assertEquals("count", config.model());
    assertEquals("[a-z]+", config.pattern());
    assertEquals("café", config.haystack());
    assertArrayEquals(haystack, config.haystackBytes());
    assertEquals(5, config.maxIters());
    assertEquals(2, config.maxWarmupIters());
    assertEquals(3_000_000L, config.maxTime());
    assertEquals(1_000_000L, config.maxWarmupTime());
    assertEquals(
        Pattern.CASE_INSENSITIVE | Pattern.UNICODE_CASE | Pattern.UNICODE_CHARACTER_CLASS,
        config.compileRegex().flags());
  }

  @Test
  void leavesUtf8HaystackAsBytesForLaterValidation() throws Exception {
    byte[] malformed = {(byte) 0xC3, (byte) 0x28};
    BenchmarkConfig config = BenchmarkConfig.parse(validKlv(malformed), InputMode.UTF8);

    assertNull(config.haystack());
    assertArrayEquals(malformed, config.haystackBytes());
    assertThrows(IllegalArgumentException.class, () -> Utf8Input.validated(config.haystackBytes()));
    assertThrows(
        CharacterCodingException.class,
        () -> BenchmarkConfig.parse(validKlv(malformed), InputMode.STRING));
  }

  @Test
  void rejectsMalformedKlvLengthsAndText() {
    assertThrows(
        IllegalArgumentException.class,
        () ->
            BenchmarkConfig.parse(
                "model:99:count\n".getBytes(StandardCharsets.UTF_8), InputMode.STRING));

    ByteArrayOutputStream bytes = new ByteArrayOutputStream();
    write(bytes, "model", new byte[] {(byte) 0xC3, (byte) 0x28});
    assertThrows(
        CharacterCodingException.class,
        () -> BenchmarkConfig.parse(bytes.toByteArray(), InputMode.STRING));
  }

  @Test
  void rejectsMultiplePatterns() {
    ByteArrayOutputStream bytes = new ByteArrayOutputStream();
    bytes.writeBytes(validKlv(new byte[0]));
    write(bytes, "pattern", "second");
    assertThrows(
        IllegalArgumentException.class,
        () -> BenchmarkConfig.parse(bytes.toByteArray(), InputMode.STRING));
  }

  private static byte[] validKlv(byte[] haystack) {
    ByteArrayOutputStream bytes = new ByteArrayOutputStream();
    write(bytes, "model", "count");
    write(bytes, "pattern", "[a-z]+");
    write(bytes, "case-insensitive", "true");
    write(bytes, "unicode", "true");
    write(bytes, "haystack", haystack);
    write(bytes, "max-iters", "5");
    write(bytes, "max-warmup-iters", "2");
    write(bytes, "max-time", "3000000");
    write(bytes, "max-warmup-time", "1000000");
    return bytes.toByteArray();
  }

  private static void write(ByteArrayOutputStream output, String key, String value) {
    write(output, key, value.getBytes(StandardCharsets.UTF_8));
  }

  private static void write(ByteArrayOutputStream output, String key, byte[] value) {
    output.writeBytes((key + ":" + value.length + ":").getBytes(StandardCharsets.UTF_8));
    output.writeBytes(value);
    output.write('\n');
  }
}
