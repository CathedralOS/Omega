# Literal Suffixes

Compile-scope probe: typed integer literal suffixes. No
standard-library dependency.

- `100i64`, `7u32`, `9usize` — the suffix selects the literal's
  primitive type; each value round-trips through storage and
  compares equal to the same suffixed literal.
- `usize` is its own primitive.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/literal_suffixes/main.omg
mbx run -p omega -- update --project samples/cli/probes/literal_suffixes
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.
