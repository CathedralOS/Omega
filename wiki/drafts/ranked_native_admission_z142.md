# RANKED-NATIVE-ADMISSION — scope verification (2026-09-20, `797e99ead7`)

Mined stub at `TASKS.md` (`**RANKED-NATIVE-ADMISSION** — mined candidate;
verify scope then implement`). **Scope verified: the name re-mines the
GENERAL-CYCLIC-EXECUTION-OPTIMIZER surface and carries no independent,
unclaimed slice this wave.**

## What the name refers to

`omega-rust/omega/pipeline/terminal-psi-to-abstract-operations/src/artifact_admission/native.rs`
— "Ranked native admission" in the crate README. Verified at `797e99ead7`:
`lower_decoded_native_module` verifies the module and lowers **the whole
verified artifact** into one `AbstractOperationPlan`
(`AdmittedNativeArtifact`). There is no per-callee call/return composition:
no `AcceptedControlCycle`-level roster beyond the artifact, no composed
argument references, no callee measure checking. That matches the sibling
row `RANKED-CALLEE-NATIVE-COMPOSITION`'s verified reading — the open work is
composed call/return/cleanup/measure/resource evidence for ranked callees
reaching native lowering, an extend-the-common-graph leg on the parent
optimizer rows, not a separate item.

## Why no slice is workable this wave

Every implementing surface for that leg is inside a live claim:

- `typed-trees-to-checked-trees/src/execution/terminal_unit/{types,calls,control,mod.rs}`,
  `checked-trees-to-lowered-psi/src/unit`, `lowered-psi-to-terminal-psi/src`,
  `terminal-psi-to-abstract-operations/src`,
  `abstract-operations-to-target-operations/src/lowering`,
  `terminal-psi`/`target-operations` representations →
  **PLACED-ACCESS-NATIVE-OPS** (Devin / z152, expires 2026-09-21T00:56Z)
- `execution/terminal_unit/receiver_calls`, `tests/borrow`,
  `lowering/unit/structural_call.rs` → **STRUCTURAL-BORROW-IDENTITY**
  (expires 2026-09-20T21:38Z)
- `execution/terminal_unit/mod.rs` → **UEFI-OS-HANDOFF** (expires 2026-09-20T20:00Z)
- `abstract-operations-to-target-operations/src/lowering/control_flow{,.rs}`
  → **STRUCTURAL-UNIT-CALL-GRAPH-JOINS** (expires 2026-09-20T20:13Z)
- `execution/terminal_unit/structural_scalar_store` → **PSI-NATIVE-FIELD-STORES**
  (expires 2026-09-21T02:12Z)

The `RANKED-PROJECTED-RECEIVER-COMPOSITION` row (verified `6ef64f6dd6`)
already recorded this same surface against the earlier fence set; the fences
have only grown since.

## Outcome

No code change made; the residual stays on the parent optimizer rows
(GENERAL-CYCLIC-EXECUTION-OPTIMIZER / TASKS_OPTIMIZER.md). Coordinator may
annotate the stub as verified-re-mine alongside
RANKED-CALLEE-NATIVE-COMPOSITION and RANKED-PROJECTED-RECEIVER-COMPOSITION.
