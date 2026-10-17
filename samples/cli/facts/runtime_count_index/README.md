# Runtime count index

A byte view is indexed by a runtime count. The count is a local, a record
field, a parameter, and a call result. The reads happen in a state reached
after the array is created. Bytes `[9, 8, 7, 6]` produce `8`, `9`, `6`, and
`7`. Exit 0 means all four reads matched. A mismatch traps.

```sh
omega run samples/cli/facts/runtime_count_index/main.omg
```

Expected exit: 0.

The unknown-field control is
`tests/omega/fail/generics/value_binder_unknown_field`. A `const` binder still
rejects a runtime name, as in
`tests/omega/fail/generics/const_generic_runtime_argument`.
