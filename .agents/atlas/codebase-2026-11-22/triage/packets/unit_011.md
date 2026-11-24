# Atlas packet unit_011: stage 04 target lowering and stage 05 selection helpers

Status: in progress. The first receipt is verified; the rest of the slice is under review.

This packet gives one disposition per correctness claim in
[record unit_011](../../records/unit_011.md) and the matching lines of the
[area 10 digest](../../digests/10-native-stages-04-10.md). The record read
`4f3c255d`; evidence here is read from `main` at
`24ad2700685e6cc6062026c87ed5fd24ed415265` (current line numbers may differ
from the record's). No supplemental range in `second_pass_ranges.tsv` belongs
to unit 11, so no gap packet owns part of this slice.

Paths shorten
`source-rust/subsystems/compiler/psi-to-native/pipeline/04_abstract-operations-to-target-operations/src/target_lowering/`
to `S04/` and
`source-rust/subsystems/compiler/psi-to-native/pipeline/05_target-operations-to-selected-instructions/src/`
to `S05/`.

## Receipts

| # | Claim (original location) | Current source | Contract, test or trace | Outcome |
| --- | --- | --- | --- | --- |
| 1 | `derive_mixed_structural_scalar_function_abi` turns a failed `prepare_function_signature` into "no mixed ABI" (record `function_signature.rs` fail-open 1; digest §5 line 132, §6 line 206, §10 item 13) | `S04/function_signature.rs:189-195`; `S04/coordination.rs:533-540`, `:644-684`; `S04/mod.rs:251-278`; `S04/control_flow.rs:2855-2874` | Coordination calls `lower_function` for every function before it attaches the mixed ABI. `lower_function` either refuses or calls `control_flow::lower`, which either refuses or calls `prepare_function_signature(function, target, structural_types)?` with the same three inputs, so the same error stops the plan. | Duplicate of the merged [07–13 ledger](../07-13-psi-and-native.md) row "10, mixed ABI detection" (refusal preserved) |
