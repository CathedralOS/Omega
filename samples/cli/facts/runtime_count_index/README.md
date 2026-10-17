# Runtime count index

A byte view is indexed by a runtime count. Bytes `[9, 8, 7, 6]` indexed at
`1` produce `8`. Exit 0 means that read matched. A mismatch traps.

The count is one value-binder argument. Each directory is one place, the same
row as `samples/cli/facts/length/`: a local, a field, a parameter, a call
result, a match join, a const name, another state, another machine, and
another target body.

```sh
omega run samples/cli/facts/runtime_count_index/local/main.omg
omega run samples/cli/facts/runtime_count_index/field/main.omg
omega run samples/cli/facts/runtime_count_index/parameter/main.omg
omega run samples/cli/facts/runtime_count_index/call_result/main.omg
omega run samples/cli/facts/runtime_count_index/match_join/main.omg
omega run samples/cli/facts/runtime_count_index/const_name/main.omg
omega run samples/cli/facts/runtime_count_index/other_state/main.omg
omega run samples/cli/facts/runtime_count_index/other_machine/main.omg
omega run samples/cli/facts/runtime_count_index/target_body/main.omg
```

Expected exit: 0.

The unknown-field control is
`tests/omega/fail/generics/value_binder_unknown_field`. A `const` binder still
rejects a runtime name, as in
`tests/omega/fail/generics/const_generic_runtime_argument`.
