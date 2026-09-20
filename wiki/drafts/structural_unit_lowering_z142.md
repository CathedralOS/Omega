# STRUCTURAL-UNIT-LOWERING — verification record (2026-09-20, `ff2f489bbf`)

Marked row at `TASKS.md:6233` already scope-verified on `a4ffd1aff8`.
Re-verified at `ff2f489bbff725be8375ae9744abc374441c0314`:

- Landed subset confirmed:
  `checked-trees-to-lowered-psi/src/unit/structural_unit_control.rs` exists —
  multi-state claim-free affine structural control, two-frontier joins,
  ranked `TerminalRankedScc` countdown; structural arguments also ride
  `scalar_structural_calls` / `expression_preparation` bindings.
- Remaining gaps are deliberate capacity fences inside `src/unit/`
  (≤2 checked conditional states, exactly-2-frontier joins + ≤1 join
  state, acyclic outside the ranked countdown lane, specialized scalar
  successors accept only parameter-sourced arguments).

## Fences (live at verification)

| Surface | Claim | Expiry |
| --- | --- | --- |
| `c2l/src/unit` (wholesale) | UEFI-OS-HANDOFF — Devin / zergling-148 | 2026-09-21T05:13Z |
| `terminal-psi-to-abstract-operations/src/lowering/control_flow` (the `UnsupportedControlFlow(MachineId(..))` native continuation the five `structural_units` pipeline-ownership legs stop at) | STRUCTURAL-UNIT-CALL-GRAPH-JOINS — Devin / z37-structural-unit-call-graph-joins | 2026-09-21T04:33Z |

No independent unclaimed slice exists this wave; the residual stays on
the claim holders above. Record only; no code change.

Coordinator: keep the row open — residual fences belong to the two named
lanes.
