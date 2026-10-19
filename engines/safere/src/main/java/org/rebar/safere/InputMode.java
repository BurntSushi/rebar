package org.rebar.safere;

enum InputMode {
  STRING("string"),
  UTF8("utf8");

  private final String argument;

  InputMode(String argument) {
    this.argument = argument;
  }

  String argument() {
    return argument;
  }

  static InputMode parse(String argument) {
    return switch (argument) {
      case "string" -> STRING;
      case "utf8" -> UTF8;
      default -> throw new IllegalArgumentException("unknown SafeRE input mode: " + argument);
    };
  }
}
