# Field `where` index

A data `where` roster carries standing field facts. `0 <= k, k <= 3`
holds for every `Main` value, so `arr[k + 1]` into an array of length 5
compiles with no guard. The indexing machine does not see the literal
store of `k`; the where fact is the proof.

All nine places verify at runtime (`Expected exit: 0` with `crashes Trap`) that
`arr[k + 1]` reads array elements 40 (at `k = 2`) and 50 (at `k = 3`).

Nine places, one fact:

- `local/` — the field copied into a local
- `field/` — the field itself, from the entry and again after a transition
- `parameter/` — the field passed to a parameter whose `requires` is the bound
- `call_result/` — a call that returns the field and `ensures` the bound
- `match_join/` — a match whose arms are both the field
- `const_name/` — the ceiling named by `const LIMIT`
- `other_state/` — a later state parameter carrying the bound
- `other_machine/` — another machine producing and consuming the bound
- `target_body/` — the same field index in every target body

Each place has `invalid/`. Raising the upper bound to `k <= 4` allows `index + 1 == 5`
out of bounds for `[i32 in Wrapping; 5]` and rejects with `cannot prove index ... is within length 5`.

```sh
mbx run -p omega -- --check samples/cli/facts/field_where_index/local/main.omg
mbx run -p omega -- --check samples/cli/facts/field_where_index/field/main.omg
mbx run -p omega -- --check samples/cli/facts/field_where_index/parameter/main.omg
mbx run -p omega -- --check samples/cli/facts/field_where_index/call_result/main.omg
mbx run -p omega -- --check samples/cli/facts/field_where_index/match_join/main.omg
mbx run -p omega -- --check samples/cli/facts/field_where_index/const_name/main.omg
mbx run -p omega -- --check samples/cli/facts/field_where_index/other_state/main.omg
mbx run -p omega -- --check samples/cli/facts/field_where_index/other_machine/main.omg
mbx run -p omega -- --check samples/cli/facts/field_where_index/target_body/main.omg
```
