# REPOSITORY-BASELINE-GATE — z70 spot re-verification

**Revision:** `f72122f71e` (origin/main tip, linux x86_64 host).
**Scope exercised:** the two cheapest baseline legs plus the arch pair the RC-REPOSITORY-GATE slot this session already touched. The full five-command sweep (clippy --workspace, cargo check --workspace, nextest --workspace --lib) was NOT re-run — its per-command attribution is already recorded on the sibling rows (RC-REPOSITORY :9091 at `a9fa1a4fe6`, rc_repository_baseline_linux_x86_64 at `f1e9a3733d`) and every named repair leg is fenced.

## Legs

| Command | Result | Delta |
| --- | --- | --- |
| `python tools/fmt.py --check` | **GREEN** | Was red at `a9fa1a4fe6` and `2e5d4a73247` (drift in `checked-interpreter/tests/trait_operators.rs`, import ordering). The drift has been repaired on main — the fmt leg is currently green. |
| `cargo nextest run -p omega-architecture-test -E 'test(~glob_self_imports) or test(~custody_field_inventory)'` | 0/2, attributed | `glob_self_imports` narrowed to 2 offenders (`extents`, `validation` — both fenced: DEVICE-EXTENT-ACCESS exp 11:04Z, MATCH-SELECTIVE-LOWERING exp 07:39Z); the two unfenced offenders were repaired this session at `e2cbbf7f21` (RC-REPOSITORY-GATE slot). `custody_mutation_matrix` down to the single residual `compilation-report/.../custody_tests.rs` (PCC evidence-family lane). |

## Reading

The row's adjudication stands: every remaining red leg is a fenced repair row's work (DEVICE-EXTENT-ACCESS, MATCH-SELECTIVE-LOWERING, the PCC custody-matrix lane, BUILD-PACKAGES-GATE, PROOF-RULE-CLASSICALITY-AUDIT, `Service<R>`-fixture families). The fmt leg crossed red→green on main without this row's help, confirming the gate is a measure of the repair rows' convergence — not an implementable slice itself. No unclaimed repair surface exists on this host.

TASKS.md stayed under TASKS-only claims this slot; this draft carries the ledger. Row stamped with the leg deltas.
