# Field-selected rank and fixed endpoint

The original recursive walk decrements `Countdown.remaining` while preserving
its `ceiling` argument. The hosted driver invokes that unchanged walk with five
and ceiling nine, then traps unless the complete countdown returns zero.
Native and Terminal execution must both exit zero.

```sh
omega run --both tests/omega/pass/termination/measure_field_rank_endpoint/main.omg
python3 tools/corpus_gate.py --both --filter termination/measure_field_rank_endpoint,termination/rank_arrival_unobserved_array_extent_compile,borrows/runtime_projected_fixed_byte_array_local_exit
python3 tools/corpus_gate.py --filter termination/measure_field_arrival_contract,termination/measure_field_limit_changed,termination/measure_field_computed_limit_overflow
```

The public command uses ordinary package/entry review; no acceptance policy is
included. Runtime qualification here is macOS ARM64. Other Build bindings do
not claim matching-host execution.

## Historical scalar declarations

Nine declarations across this group retain their bounds as ordinary
propositions. Every carrier remains `u64`; it supplies the original zero floor
on fields. The one nonzero parameter floor remains explicit.

| Original revision and fixture | Original positions | Canonical obligation |
|---|---|---|
| `a16c1ae91f0`, `measure_field_rank_endpoint` | `Countdown.remaining: [0..=5]`; `walk.ceiling: [5..=10]` | data `where remaining <= 5`; `requires ceiling >= 5 && ceiling <= 10` |
| `fba85426683`, `rank_arrival_unobserved_array_extent_compile` | `Countdown.remaining: [0..=5]` | data `where remaining <= 5` |
| `c3300162d8b`, `measure_field_arrival_contract` | `Countdown.{remaining,limit,spare}: [0..=5]` | three containing-data upper bounds |
| `c3300162d8b`, `measure_field_limit_changed` | `Countdown.{remaining,limit}: [0..=5]` | two containing-data upper bounds |
| `95171450b11`, `measure_field_computed_limit_overflow` | `Countdown.remaining: [0..=5]` | data `where remaining <= 5`; `limit` was and remains bare `u64` |

The measured field and rank-window expressions stay unchanged. A restored
bounded value does not conserve a different endpoint; the limit replacement
still rejects. Replacing `spare` with zero does not establish the recursive
call's original relational requirements. An unbounded `limit` does not make
`limit + 1` safe Exact arithmetic. None of these controls is rewritten to pass.

The older `measure_field_rank_endpoint_replaced` companion had the same two
declarations as this positive at `a16c1ae91f0`; it was deleted by the corpus
deduplication at `09add20600f`, not migrated to bare unconstrained fields.
The retained `measure_field_limit_changed` control exercises the same rule:
replacing the endpoint with five stays within its bound but loses its identity.

A mechanical probe carries the endpoint through a record field, a helper's
exact result guarantee and a local; both engines retain zero. Supplying four
instead refuses the ordinary lower-bound requirement. These probes are
restored before landing. No interval-only domain replaces a proposition.
