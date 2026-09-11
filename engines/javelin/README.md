This directory provides a Rust runner for JavelinPattern using the
[`javelin-pattern` crate](https://crates.io/javelin-pattern). The runner is
invoked by `javelin` and `javelin/bt` engines. The only difference in the
latter case is that the `--backtracking` option is enabled, which corresponds
to setting the `JP_OPTION_PREFER_BACKTRACKING` flag when compiling the pattern.

When Unicode is requested, the following additional flags are set:

- `JP_OPTION_UTF8`
- `JP_OPTION_UNICODE_CASE`
- `JP_OPTION_UCP`

The runner's package version matches its pinned `javelin-pattern` version so
`--version` identifies the dependency being measured. They should be updated together.
