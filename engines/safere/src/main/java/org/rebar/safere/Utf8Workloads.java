package org.rebar.safere;

import java.io.ByteArrayOutputStream;
import java.nio.charset.StandardCharsets;
import org.safere.Pattern;
import org.safere.Utf8Input;
import org.safere.Utf8Matcher;

/** Implements Rebar workload models using SafeRE's UTF-8 API. */
final class Utf8Workloads {
  private static final String REGEX_REDUX_EXPECTED =
      """
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

  private Utf8Workloads() {}

  static Workloads.Workload create(BenchmarkConfig config) {
    byte[] haystack = config.haystackBytes();
    Utf8Input input = Utf8Input.validated(haystack);
    return switch (config.model()) {
      case "count" -> count(config.compileRegex(), input);
      case "count-spans" -> countSpans(config.compileRegex(), input);
      case "count-captures" -> countCaptures(config.compileRegex(), input);
      case "grep" -> grep(config.compileRegex(), haystack, false);
      case "grep-captures" -> grep(config.compileRegex(), haystack, true);
      case "regex-redux" -> () -> regexRedux(config, haystack);
      default ->
          throw new IllegalArgumentException("unrecognized benchmark model: " + config.model());
    };
  }

  static int countMatches(Pattern pattern, Utf8Input input) {
    int count = 0;
    Utf8Matcher matcher = pattern.matcher(input);
    while (matcher.find()) {
      count++;
    }
    return count;
  }

  private static Workloads.Workload count(Pattern pattern, Utf8Input input) {
    return () -> countMatches(pattern, input);
  }

  private static Workloads.Workload countSpans(Pattern pattern, Utf8Input input) {
    return () -> {
      int sum = 0;
      Utf8Matcher matcher = pattern.matcher(input);
      while (matcher.find()) {
        sum += matcher.end() - matcher.start();
      }
      return sum;
    };
  }

  private static Workloads.Workload countCaptures(Pattern pattern, Utf8Input input) {
    return () -> {
      int count = 0;
      Utf8Matcher matcher = pattern.matcher(input);
      while (matcher.find()) {
        for (int group = 0; group <= matcher.groupCount(); group++) {
          if (matcher.start(group) >= 0) {
            count++;
          }
        }
      }
      return count;
    };
  }

  private static Workloads.Workload grep(Pattern pattern, byte[] haystack, boolean captures) {
    return () -> {
      int count = 0;
      for (int lineStart = 0; lineStart < haystack.length; ) {
        int lineEnd = lineStart;
        while (lineEnd < haystack.length && haystack[lineEnd] != '\n') {
          lineEnd++;
        }
        int contentEnd = lineEnd;
        if (contentEnd > lineStart && haystack[contentEnd - 1] == '\r') {
          contentEnd--;
        }
        Utf8Input line = Utf8Input.trusted(haystack, lineStart, contentEnd - lineStart);
        if (captures) {
          Utf8Matcher matcher = pattern.matcher(line);
          while (matcher.find()) {
            for (int group = 0; group <= matcher.groupCount(); group++) {
              if (matcher.start(group) >= 0) {
                count++;
              }
            }
          }
        } else if (pattern.find(line)) {
          count++;
        }
        lineStart = lineEnd < haystack.length ? lineEnd + 1 : lineEnd;
      }
      return count;
    };
  }

  private static int regexRedux(BenchmarkConfig config, byte[] haystack) {
    byte[] sequence = haystack;
    int inputLength = sequence.length;
    sequence = replaceAll(config, sequence, ">[^\n]*\n|\n", "");
    int cleanedLength = sequence.length;

    StringBuilder result = new StringBuilder();
    String[] variants = {
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
    for (String variant : variants) {
      int count = countMatches(config.compilePattern(variant), Utf8Input.trusted(sequence));
      result.append(variant).append(' ').append(count).append('\n');
    }

    sequence = replaceAll(config, sequence, "tHa[Nt]", "<4>");
    sequence = replaceAll(config, sequence, "aND|caN|Ha[DS]|WaS", "<3>");
    sequence = replaceAll(config, sequence, "a[NSt]|BY", "<2>");
    sequence = replaceAll(config, sequence, "<[^>]*>", "|");
    sequence = replaceAll(config, sequence, "\\|[^|][^|]*\\|", "-");

    result
        .append('\n')
        .append(inputLength)
        .append('\n')
        .append(cleanedLength)
        .append('\n')
        .append(sequence.length)
        .append('\n');
    if (!result.toString().trim().equals(REGEX_REDUX_EXPECTED.trim())) {
      throw new IllegalStateException("regex-redux result did not match expected output");
    }
    return sequence.length;
  }

  private static byte[] replaceAll(
      BenchmarkConfig config, byte[] input, String regex, String replacement) {
    Utf8Matcher matcher = config.compilePattern(regex).matcher(Utf8Input.trusted(input));
    Utf8Input replacementInput = Utf8Input.trusted(replacement.getBytes(StandardCharsets.UTF_8));
    ByteArrayOutputStream output = new ByteArrayOutputStream(input.length);
    while (matcher.find()) {
      matcher.appendReplacement(output::write, replacementInput);
    }
    matcher.appendTail(output::write);
    return output.toByteArray();
  }
}
