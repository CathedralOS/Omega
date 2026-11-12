# Route coverage shard 1

PIN: `0835b3aaf803ed97f905be42d94727ff58459263`
Harness built at: `198b8cbff01c683fc49157e03474b5d3857c0765`
Rows completed: 2174 (shard size 2174)

## Execution

Serial one `route_one_comparison` per fixture (`tools/build_slot.md`).
Artifacts live under `.agents/four-jobs-2026-11-12/route-coverage/`.

## Route verdict rules

- **accepted**: every harness `program` root is `route=accepted`.
- **partial**: ≥1 root `accepted` and ≥1 root outside / unjudged / harness error.
  Not route acceptance; excluded from agree/disagree.
- **rejected** / **outside** / **error** / **timeout** as documented.

`current_verdict` = harness `current=` on the typed-frontier check only.
`stopped` = parse/type failure before checker (not a rejection).

## Totals

| bucket | count |
| --- | ---: |
| outside | 1863 |
| accepted | 38 |
| rejected | 65 |
| partial | 208 |
| error | 0 |
| timeout | 0 |

Agreement (judged n=103): agree 87, 
disagree 16, n-a 2071.

**Soundness watch** (route=accepted, current=rejected): **0**.

## Soundness watch breakdown

None under strict acceptance. Former tally-style “accepted with outside siblings” rows are now **partial** (208).

## Disagreements (route vs checker)

- count: 16; by tier: {'pass': 16}
- All 16 are `pass/` positives: **route=rejected**, **current=accepted** (route stricter, not soundness watch).

## Hand-check: five rows (soundness watch candidates)

1. `tests/omega/pass/arithmetic/runtime_boolean_alias_shift_count_exit/main.omg`
   - Route rejects (unproved arrival/crash obligations); checker accepted. **Not** soundness watch — lowered route is stricter on a corpus pass.

1. `tests/omega/pass/facts/bounded_products_through_other_state_exit/main.omg`
   - Same pattern on a `facts/` exit twin: route rejected, checker accepted. Not a hole (inverse disagreement).

1. `tests/omega/pass/facts/match_join_multi_variant_bound_through_other_state_exit/main.omg`
   - Route rejected with lowered obligations; checker accepted. Not watch.

1. `tests/omega/fail/arithmetic/immutable_unsigned_mask_false_floor_rejected/main.omg`
   - Now **partial** (helper accepted + defect machine outside). Was mis-counted as route-accept before; **not** watch.

1. `tests/omega/fail/calls/generic_operator_guard_argument_mismatch/main.omg`
   - **partial** after fix; checker rejected on `inspect` (type float) while `main`/`done` accepted on route. Not watch.

## Top first-refusal constructs (outside bucket)

- `machine service reach` — 333
- `meta:no-bound-entry` — 267
- `parameter mutable parameter` — 257
- `machine generic parameters` — 187
- `type reference` — 152
- `declaration Build selection that is not a machine or a data type` — 119
- `type data with custody` — 102
- `machine attached data` — 84
- `type constrained` — 58
- `type float` — 34