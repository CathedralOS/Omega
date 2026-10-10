# cross width compare

Compile-scope probe. No standard-library dependency.

- Integer comparisons compose across carrier widths: `i8 != i32`,
  `i16 < i64`, `u8 == i8` all on data fields.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/cross_width_compare/main.omg
mbx run -p omega -- update --project samples/cli/probes/cross_width_compare
```
