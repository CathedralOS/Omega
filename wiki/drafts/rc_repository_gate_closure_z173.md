# RC-REPOSITORY-GATE-CLOSURE — linux x86-64 leg (z173)

Record of the `RC-REPOSITORY` release gate block from
[rust_compiler_completion.md](rust_compiler_completion.md#release-matrix) run
on linux x86_64 (swarm VM, cargo — `mbx` unavailable) at commit
`96b4afed92` (origin/main, 2026-09-20). This leg re-measures the gate after
`a1daf35f2e` ([rc_repository_gate_closure_z142.md](rc_repository_gate_closure_z142.md))
and repairs the unfenced fmt drift.

**Gate does not close: all five commands are red at this head.** The dominant
blocker is a compile break introduced by `2d8c5136cc` (still at tip): the
workspace no longer builds, so commands 2, 4, and 5 cannot start. Every
remaining failure — fmt residual included — sits inside a live sibling claim.

| # | Command | Exit | Result |
| --- | --- | --- | --- |
| 1 | `cargo fmt --all -- --check` | 1 | 53 hunks across 20 files at measurement; this leg formatted the 17 unclaimed files — residual is exactly the 4 claim-fenced files below |
| 2 | `cargo clippy --workspace --all-targets -- -D warnings` | 101 | compile-blocked: E0432 `unresolved import effects::ComponentEraJournalRoster` at `external-roots/src/program_local/program_local_roots/epoch_cohorts.rs:9` (reproducer: `cargo check -p external-roots`) |
| 3 | `cargo nextest run -p omega-architecture-test --all-targets --no-fail-fast` | 100 | 574 tests: 560 passed, 14 failed (was 573 tests / 13 failed at `a1daf35f2e`) |
| 4 | `cargo check --workspace --all-targets` | 101 | compile-blocked by the same E0432 in `external-roots`; earlier `pipeline_ownership` drift is masked and unverifiable this leg |
| 5 | `cargo nextest run --workspace --lib --no-fail-fast` | 101 | compile-blocked by the same E0432 |

## 1. fmt drift — 20 files at measurement

Unclaimed (17 + 1 cascade) — **formatted by this leg** (branch `zergling/z173`):

- `external-roots/src/interrupts/interrupt_table/{member_admissions,tests,
  member_admission_and_publication, checked_publication_authority}.rs`,
  `external-roots/src/stack_and_fuel/stack_demand.rs`,
  `compiler/tests/canary_suite/task_runtime.rs`,
  `compiler/tests/layout_plans/interrupt_descriptor_tables.rs`,
  `compiler/tests/module_machine_indices/comparisons.rs`,
  `target-operations-to-selected-instructions/src/tests/legalization/
  dynamic_parameter_call.rs`,
  `terminal-psi-to-abstract-operations/src/lowering/machine/operation/
  structural_scalar_fields.rs`,
  `calling-conventions/src/lib.rs`,
  `omega/tests/inspect_terminal/integer_comparison_custody.rs`,
  `typed-trees-to-checked-trees/src/checks/termination/progress/
  origins{,/tests,/tests/references}.rs`,
  `typed-trees-to-checked-trees/tests/token_bound_machine_calls.rs`,
  `tests/architecture/optimizer_source_organization/inventory.rs`.

(z142 had fenced most of these under RC-REPOSITORY-BASELINE-GREEN,
RANKED-NATIVE-ADMISSION, CORPUS-RED-FAMILY-CASTSEED, and
EXCEPTION-ROOTS-AND-TIMER; those claims have since released, so the files
were repairable this leg. This leg holds a fresh `fmt-drift` sub-claim over
them for the duration of the edit.)

Claim-fenced residual (4) — left untouched:

- `typed-trees-to-checked-trees/src/checks/multiplicity/{borrowed_windows,
  linear_obligations}.rs` → ADDRESS-TRANSLATION-CANARY (lease to 00:18Z)
- `typed-trees-to-checked-trees/src/execution/terminal_unit/
  structural_scalar_store/tests/mod.rs` → PSI-NATIVE-FIELD-STORES (02:12Z)
- `checked-interpreter/tests/trait_operators.rs` → NAMED-TRAIT-OPERATORS
  (01:45Z)

## 2–5. Workspace build break — dominant blocker

`2d8c5136cc` ("backend: compose epoch aggregate snapshots against the
journal-replayed roster") re-imports `effects::ComponentEraJournalRoster`,
which `20bd592af1` had deleted along with `component_era_journal.rs` /
`ComponentEraJournal::replay` (~900 lines). `cargo check -p external-roots`
reproduces:

```
error[E0432]: unresolved import `effects::ComponentEraJournalRoster`
 --> omega-rust/omega/backend/runtime/external-roots/src/program_local/
     program_local_roots/epoch_cohorts.rs:9
```

Every workspace-scoped command (2, 4, 5) fails there. The repair — a revert
of `2d8c5136cc`, since fix-forward would resurrect the deliberately-removed
journaling machinery — is fenced to MAIN-BROKEN-BASE-EPOCH-COHORTS (owner
`dev-l3-basefix`, lease to 2026-09-21T05:04Z, six paths incl.
`epoch_cohorts.rs`, `program_local_roots.rs`, `lib.rs`).

## 3. architecture — 14 failures

Suite compiles (does not link `external-roots`); ran clean:

- `validation_integration` × 11 — the in-flight RECAST/boundary-ensures lane:
  `boundary_ensures_{equalities_couple_symbolic_recast_witnesses,
  witness_discharges_recast_footprint, witness_survives_unrelated_internal_call,
  witness_survives_unrelated_intervening_call, witness_too_wide_refuses_recast_
  footprint}`, `boundary_witness_survives_transitive_disjoint_boundary_frame`,
  `admitted_provider_receiver_receipt_removes_build_bound_demand`,
  `checked_progress_retains_provider_receiver_as_build_bound_demand`,
  `symbolic_walk_{recast_footprint_discharges, weak_guard_spelling_refuses,
  recast_wide_witness_refuses}` — all now reject with the footprint-bounding
  diagnostic rather than the asserted one
- `glob_self_imports_never_grow_per_crate` — the ratchet (sibling glob legs
  in flight)
- `layering abstract_to_target_translation_validation_cannot_reenter_its_
  producer`, `representation_ownership
  exit_replay_checks_claimed_records_without_reentering_the_producer`

## Host rows

Only the linux x86-64 leg exists. The macos arm64, windows x64, and the
fourth host row remain host-gated.
