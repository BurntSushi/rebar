package org.rebar.safere;

import static org.junit.jupiter.api.Assertions.assertEquals;

import java.io.ByteArrayOutputStream;
import java.nio.charset.StandardCharsets;
import org.junit.jupiter.api.Test;

final class WorkloadsTest {
  @Test
  void grepUsesLfAndCrLfWhilePreservingEmbeddedCr() throws Exception {
    for (InputMode mode : InputMode.values()) {
      assertEquals(2, run(mode, "grep", "", "a\r\na\ra\n"), mode.name());
      assertEquals(1, run(mode, "grep", "^a$", "a\r\na\ra\n"), mode.name());
      assertEquals(6, run(mode, "grep-captures", "(a)", "a\r\na\ra\n"), mode.name());
      assertEquals(2, run(mode, "grep-captures", "(\\r)", "a\ra\n"), mode.name());
      assertEquals(1, run(mode, "grep", "^a$", "a\r"), mode.name());
      assertEquals(0, run(mode, "grep", "", ""), mode.name());
      assertEquals(2, run(mode, "grep", "", "\n\n"), mode.name());
    }
  }

  @Test
  void spansFollowTheSelectedInputRepresentation() throws Exception {
    assertEquals(2, run(InputMode.STRING, "count-spans", ".", "😀"));
    assertEquals(4, run(InputMode.UTF8, "count-spans", ".", "😀"));
  }

  private static int run(InputMode mode, String model, String pattern, String haystack)
      throws Exception {
    ByteArrayOutputStream input = new ByteArrayOutputStream();
    write(input, "model", model);
    write(input, "pattern", pattern);
    write(input, "haystack", haystack);
    return Workloads.create(BenchmarkConfig.parse(input.toByteArray(), mode), mode).run();
  }

  private static void write(ByteArrayOutputStream input, String key, String value) {
    byte[] bytes = value.getBytes(StandardCharsets.UTF_8);
    input.writeBytes((key + ":" + bytes.length + ":").getBytes(StandardCharsets.UTF_8));
    input.writeBytes(bytes);
    input.write('\n');
  }
}
