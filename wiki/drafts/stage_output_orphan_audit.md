# Stage-output orphan audit

Companion sweep to `stage_entrance_orphan_audit.md`. The entrance audit answers
"does a designed stage entrance have a caller"; this audit answers the
placement rule's other half — once a stage produces a route, does a
coordinator or successor actually consume it. A stage whose produced route no
listed consumer names would still pass every entrance check, so the guard
lands beside them in
`tests/architecture/optimizer_source_organization/entrances/rule_stages.rs`.

## Sweep

Enumerated the five rule-owning stage descriptors in
`tests/architecture/optimizer_source_organization/inventory.rs`
(`RULE_STAGES`) and, for each, walked the produced-route consumer list —
files that must still name the stage's `output_marker`:

| Stage entrance | Produced route marker | Consumers checked |
| --- | --- | --- |
| `lowered-psi-to-lowered-psi/src/psi_optimization.rs` | `run_psi_optimization` | `terminal-production/src/terminal_production.rs` |
| `abstract-operations-to-abstract-operations/src/rules/mod.rs` | `built_in_psi_registries` | `.../pass_manager/entry.rs` |
| `selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/mod.rs` | `resolve_selected_lowering_rules` | `.../rewrites/literal_folds/mod.rs` |
| `selected-instructions-to-selected-instructions/src/rewrites/allocation_recovery/mod.rs` | `selected_allocation_recovery_rule` | `selected-instructions-to-register-homes/src/register_allocation.rs` |
| `resolved-layout-to-resolved-layout/src/x86_branch_relaxation/mod.rs` | `stage_optimized_x86_branch_relaxation` | `resolved-layout-to-resolved-layout/src/phase.rs` |

## Finding

**No orphans on `origin/main`.** Every listed consumer still names its stage's
output marker; all five stages have a live route into a coordinator or
successor file. There was nothing to repair — the outcome is a regression
guard, landed at `fddf82a61dfe`: each `RuleStageDescriptor` now carries
`output_marker` plus its `consumers` roster, and `rule_stages::check` reports
an `orphan stage output` violation when a listed consumer drops the reference
(a consumer file that can't be read is a distinct violation, not a silent
pass).

## Verification

`cargo nextest run -p omega-architecture-test -E
'test(~optimizer_source_organization)'` — `optimizer_source_organization_
preserves_semantic_owners` green at `3f5e37a0b6e1` on linux x86-64.
