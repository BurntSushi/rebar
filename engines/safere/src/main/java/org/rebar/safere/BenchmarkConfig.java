package org.rebar.safere;

import java.nio.ByteBuffer;
import java.nio.charset.CharacterCodingException;
import java.nio.charset.CharsetDecoder;
import java.nio.charset.CodingErrorAction;
import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import org.safere.Pattern;

record BenchmarkConfig(
    String model,
    String pattern,
    boolean caseInsensitive,
    boolean unicode,
    String haystack,
    byte[] haystackBytes,
    int maxIters,
    int maxWarmupIters,
    long maxTime,
    long maxWarmupTime) {

  static BenchmarkConfig parse(byte[] input, InputMode mode) throws CharacterCodingException {
    CharsetDecoder decoder =
        StandardCharsets.UTF_8
            .newDecoder()
            .onMalformedInput(CodingErrorAction.REPORT)
            .onUnmappableCharacter(CodingErrorAction.REPORT);
    String model = null;
    String pattern = null;
    String haystack = null;
    byte[] haystackBytes = null;
    boolean caseInsensitive = false;
    boolean unicode = false;
    int maxIters = 0;
    int maxWarmupIters = 0;
    long maxTime = 0;
    long maxWarmupTime = 0;

    for (int offset = 0; offset < input.length; ) {
      KlvEntry entry = readEntry(input, offset, decoder);
      offset = entry.nextOffset();
      switch (entry.key()) {
        // Rebar uses the name to identify the result outside this process.
        case "name" -> {}
        case "model" -> model = entry.value(decoder);
        case "pattern" -> {
          if (pattern != null) {
            throw new IllegalArgumentException("SafeRE runner does not support multiple patterns");
          }
          pattern = entry.value(decoder);
        }
        case "case-insensitive" -> caseInsensitive = entry.value(decoder).equals("true");
        case "unicode" -> unicode = entry.value(decoder).equals("true");
        case "haystack" -> {
          haystackBytes = entry.bytes();
          if (mode == InputMode.STRING) {
            haystack = entry.value(decoder);
          }
        }
        case "max-iters" -> maxIters = Integer.parseInt(entry.value(decoder));
        case "max-warmup-iters" -> maxWarmupIters = Integer.parseInt(entry.value(decoder));
        case "max-time" -> maxTime = Long.parseLong(entry.value(decoder));
        case "max-warmup-time" -> maxWarmupTime = Long.parseLong(entry.value(decoder));
        default -> throw new IllegalArgumentException("unrecognized KLV key: " + entry.key());
      }
    }
    if (model == null) {
      throw new IllegalArgumentException("missing benchmark model");
    }
    if (!model.equals("regex-redux") && pattern == null) {
      throw new IllegalArgumentException("missing pattern");
    }
    if (haystackBytes == null) {
      throw new IllegalArgumentException("missing haystack");
    }
    return new BenchmarkConfig(
        model,
        pattern,
        caseInsensitive,
        unicode,
        haystack,
        haystackBytes,
        maxIters,
        maxWarmupIters,
        maxTime,
        maxWarmupTime);
  }

  Pattern compileRegex() {
    return compilePattern(pattern);
  }

  Pattern compilePattern(String regex) {
    int flags = 0;
    if (caseInsensitive) {
      flags |= Pattern.CASE_INSENSITIVE;
    }
    if (unicode) {
      flags |= Pattern.UNICODE_CASE | Pattern.UNICODE_CHARACTER_CLASS;
    }
    return Pattern.compile(regex, flags);
  }

  private static KlvEntry readEntry(byte[] input, int offset, CharsetDecoder decoder)
      throws CharacterCodingException {
    int keyEnd = findColon(input, offset);
    int lengthEnd = findColon(input, keyEnd + 1);
    String key = decode(input, offset, keyEnd - offset, decoder);
    int valueLength = Integer.parseInt(decode(input, keyEnd + 1, lengthEnd - keyEnd - 1, decoder));
    int valueStart = lengthEnd + 1;
    if (valueLength < 0 || valueLength >= input.length - valueStart) {
      throw new IllegalArgumentException("invalid KLV value length for " + key);
    }
    int valueEnd = valueStart + valueLength;
    if (input[valueEnd] != '\n') {
      throw new IllegalArgumentException("missing KLV line terminator for " + key);
    }
    return new KlvEntry(key, input, valueStart, valueLength, valueEnd + 1);
  }

  private static int findColon(byte[] input, int start) {
    for (int i = start; i < input.length; i++) {
      if (input[i] == ':') {
        return i;
      }
    }
    throw new IllegalArgumentException("missing KLV delimiter");
  }

  private static String decode(byte[] input, int start, int length, CharsetDecoder decoder)
      throws CharacterCodingException {
    return decoder.decode(ByteBuffer.wrap(input, start, length)).toString();
  }

  private static final class KlvEntry {
    private final String key;
    private final byte[] input;
    private final int valueStart;
    private final int valueLength;
    private final int nextOffset;

    KlvEntry(String key, byte[] input, int valueStart, int valueLength, int nextOffset) {
      this.key = key;
      this.input = input;
      this.valueStart = valueStart;
      this.valueLength = valueLength;
      this.nextOffset = nextOffset;
    }

    String key() {
      return key;
    }

    int nextOffset() {
      return nextOffset;
    }

    String value(CharsetDecoder decoder) throws CharacterCodingException {
      return decode(input, valueStart, valueLength, decoder);
    }

    byte[] bytes() {
      return Arrays.copyOfRange(input, valueStart, valueStart + valueLength);
    }
  }
}
