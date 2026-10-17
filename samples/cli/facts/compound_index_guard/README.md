# compound index guard

Compile-scope probe. No standard-library dependency.

- wire_retired_field: `retired #3;` withdraws a published wire
  field number inside the data body.
- qualified_match_arg: domain'd match arms
  (`i64 in Coordinate<7>`) inside a call argument position.
- compound_index_guard: `b * 20 + a` two-variable index
  expression guarded once, stored unguarded.
- local_record_shadow: local `value` + `Counter{value: value}`
  field binding + `counter.value` readback; `crashes Trap`
  entry + state `crash Trap;`.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/facts/compound_index_guard/main.omg
mbx run -p omega -- update --project samples/cli/facts/compound_index_guard
```
