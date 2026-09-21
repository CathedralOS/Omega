# RC-REPOSITORY-GATE — re-verification at 3a1304c93e

Re-run of the recorded red surfaces on linux x86-64, plus the bounded repair
of the two glob-self-import offenders not fenced by sibling claims.

## Surface deltas since the recorded measure

- `tools/fmt.py --check`: GREEN.
- Scoped clippy on the validation pair: GREEN — the recorded clippy findings
  were repaired on main by 69941b6d8d.
- `omega-architecture-test` filtered run (2 tests): both still red, but the
  glob-self-imports ratchet narrowed from 4 offenders to 2 after this commit.
- Custody-mutation matrix: narrowed on main by 5a4f96e9da to a single
  compilation-report violation —
  `native_evidence/custody_tests.rs`:
  `NativePlacedImageEvidenceFieldForTest` declares a substitution field
  inventory with no `*_for_test` hook / `run_one_field_substitution_matrix`
  driver. That is the PCC native-evidence substitution-matrix lane, a
  substantive per-family implementation leg, not a bounded re-verification
  slice.

## glob_self_imports disposition (4 → 2)

| File | Disposition |
|---|---|
| `checked-compilation-to-terminal-artifact/src/terminal_artifact.rs:435` | Repaired here: `use super::merge_terminal_production_timings` (the test module's only parent-module use). |
| `build-time-evaluation/src/const_evaluation/const_applications/tests.rs:5` | Repaired here: explicit `use super::{const_application_plan, defer_pending_const_applications, evaluate_selected_const_applications, pending_const_applications_need_operator_selection}` plus direct `typed_trees` imports (they had been arriving through the parent's private `use` block). |
| `extents/src/ordering_events/tests.rs:1` | Fenced — DEVICE-EXTENT-ACCESS (z88), exp 2026-09-21T11:04Z. |
| `validation/src/value_custody/expression_types/result_type.rs:104` | Fenced — MATCH-SELECTIVE-LOWERING (Zergling-126), exp 2026-09-21T07:39Z. |

The gate is therefore not green at this tip: the two fenced imports and the
compilation-report substitution-matrix residual are each owned by their own
lanes. TASKS.md is fenced under RC-REPRESENTATIVE-PROGRAMS-GATE, so this row's
ledger rides here.

## Commands run (linux x86-64, worktree at 3a1304c93e)

```text
python tools/fmt.py --check                                   # clean
cargo check -p build-time-evaluation \
     -p checked-compilation-to-terminal-artifact --all-targets # clean
cargo nextest run -p omega-architecture-test \
  -E 'test(~glob_self_imports) or test(~custody_field_inventory)' \
  # 0/2 — see dispositions above
```
