# Declared calls on borrowed rank inputs

Owner: `NESTED-MACHINE-UNIFICATION` in [TASKS.md](../../../../../../../../TASKS.md).

> A private helper preserves the invocation rank on a tail edge, and every complete cycle strictly descends while advancing the explicit counter once.

This continues the existing borrowed-record production in the same joint-rank fact. Unlike the [owned-input shared-call production](../../declared_measure/computed_record/call/borrowed_input/main.omg), the invocation input is an exclusive reference to the caller’s original affine `Remaining` record. The declared measure lends that initialized referent to the pure, total `distance(&value)` call. Scalar proof reads do not reconstruct a record or grant ownership of its backing.

Every positive performs seven increments and an empty invocation: counter `3 → 10 → 10`, count `7 → 0` or `0 → 0`, and canaries `641` / `919` unchanged on the original caller storage. The match-join place also checks the unselected record. Each twin changes only `pending.count - 1` to `pending.count - 0`.

| Place | Positive | Nondecreasing twin |
| --- | --- | --- |
| local | [source](./main.omg) | [source](./invalid/main.omg) |
| field | [source](../../../field/borrowed_value/declared_call/main.omg) | [source](../../../field/borrowed_value/declared_call/invalid/main.omg) |
| parameter | [source](../../../parameter/borrowed_value/declared_call/main.omg) | [source](../../../parameter/borrowed_value/declared_call/invalid/main.omg) |
| call_result | [source](../../../call_result/borrowed_value/declared_call/main.omg) | [source](../../../call_result/borrowed_value/declared_call/invalid/main.omg) |
| match_join | [source](../../../match_join/borrowed_value/declared_call/main.omg) | [source](../../../match_join/borrowed_value/declared_call/invalid/main.omg) |
| const_name | [source](../../../const_name/borrowed_value/declared_call/main.omg) | [source](../../../const_name/borrowed_value/declared_call/invalid/main.omg) |
| other_state | [source](../../../other_state/borrowed_value/declared_call/main.omg) | [source](../../../other_state/borrowed_value/declared_call/invalid/main.omg) |
| other_machine | [source](../../../other_machine/borrowed_value/declared_call/main.omg) | [source](../../../other_machine/borrowed_value/declared_call/invalid/main.omg) |
| target_body | [source](../../../target_body/borrowed_value/declared_call/main.omg) | [source](../../../target_body/borrowed_value/declared_call/invalid/main.omg) |

## Requirements

- Establishment: each invocation supplies an initialized readable input and independently forms the selected measure application. The desired rank does not establish a premise of its own.
- Identity and lifetime: the complete typed coordinate retains its source reference, projections and bounds. The shared call observes the original referent; it neither copies affine contents nor transfers their custody. Writes still invalidate current observations.
- Composition: the same declared call and every-cycle descent obligation travel through all nine places. Both active and fallback paths have observable, distinct effects.
- Soundness: shared observation cannot mutate, observe write-only storage, or manufacture an owned transfer. Callee purity, totality, exact inputs and result equations remain independently checked.
- Psi 06–08: the initialized input and ordinary structural argument plan carry the original storage into the call. No statement occurrence, alternate evaluator or aggregate reconstructed from scalar leaves supplies custody.
- Omega 00–02: existing record geometry and calling plans carry the same referent and counter. Every positive binds all four hosted ProgramEntry targets, with an ordinary Build-selected implementation in target_body.
- Omega 03–06: count and both neighboring canaries are read from caller storage after the cycle; this challenges lost backing and incorrect homes. It adds no portable WCSU certification.
- Omega 07–09: acceptance requires actual Linux x86-64 native/Terminal exit-0 agreement. Other-host native execution is unmeasured.

## Evidence

Fixed fetched base: `f45f51d86f5591a3d731f4b855ee4d09187857bb`. Baseline and candidate observations are retained in the workbench’s ignored `build/a31/`. Runtime acceptance is pending; no passing result is claimed yet.

The exact positives and twins are pinned under `tests/omega/{pass,fail}/facts/nested_machine_joint_rank_borrowed_home_call_*`. Broader parent acceptance, including exclusive declaration-call arguments, affine transfers, remaining ranking forms, migration and product-producer parity, remains open.
