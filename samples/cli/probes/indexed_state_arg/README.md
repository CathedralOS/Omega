# indexed state arg

Compile-scope probe. No standard-library dependency.

- `write(items[index], output)` copies an indexed record element as a
  state argument only on the selected edge; `select(entries, 99, ..)`
  takes `false -> {}` and never evaluates `items[99]` — the guard
  guards the copy, not just the store.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/indexed_state_arg/main.omg
mbx run -p omega -- update --project samples/cli/probes/indexed_state_arg
```
