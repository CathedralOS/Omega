# RC-REPOSITORY-GATE-CLOSURE — linux x86-64 leg (z79)

Record of the `RC-REPOSITORY` release gate block from
[rust_compiler_completion.md](rust_compiler_completion.md#release-matrix) run
on linux x86_64 (swarm VM, cargo — `mbx` unavailable) at commit
`d74f2145b96` (origin/main, 2026-09-20). This leg re-measures the gate after
`96b4afed92` ([rc_repository_gate_closure_z173.md](rc_repository_gate_closure_z173.md)).

**Gate does not close: four of five commands are red; `check` is green.**
The dominant blocker from the z173 leg — the `external-roots` E0432
`ComponentEraJournalRoster` compile break introduced by `2d8c5136cc` — is
repaired at this head, so clippy, arch-test, and libtests all execute end to
end for the first time in three measurements. Every remaining failure sits
inside a live sibling claim; no unfenced drift exists at this head.

| # | Command | Exit | Result |
| --- | --- | --- | --- |
| 1 | `cargo fmt --all -- --check` | 1 | 1 drifted file: `checked-interpreter/tests/trait_operators.rs` (fenced) |
| 2 | `cargo clippy --workspace --all-targets -- -D warnings` | 101 | `permissions_set_readonly_false` at `package-source` `tree/capture/traversal.rs:513` (fenced); downstream crates unexamined past the first failure |
| 3 | `cargo nextest run -p omega-architecture-test --all-targets --no-fail-fast` | 100 | 575 tests: 572 passed, 3 failed (all fenced families) |
| 4 | `cargo check --workspace --all-targets` | 0 | **GREEN** — E0432 repaired |
| 5 | `cargo nextest run --workspace --lib --no-fail-fast` | 100 | 16170 tests: 16159 passed, 11 failed, 2 skipped (was 95 failed) |

## 1. fmt drift — 1 file

`omega-rust/psi/semantics/checked-interpreter/tests/trait_operators.rs` →
NAMED-TRAIT-OPERATORS (lease to 01:45Z Sep 21). The z173 residual set
(`t2c` multiplicity pair, `structural_scalar_store` tests) was formatted by
its claim owners in the interim; this is the only drift at this head.

## 2. clippy — first failure fenced

`permissions_set_readonly_false` at
`omega-rust/omega/packages/sources/acquisition/src/tree/capture/traversal.rs:513`
(`writable.set_readonly(false)` — on Unix the file becomes world-writable).
Fenced to BUILD-PACKAGES-GATE/acquisition-traversal-legs (lease to 02:00Z
Sep 21). The workspace compiles under `-D warnings` up to that crate;
crates later in topological order are unmeasured this leg.

## 3. arch-test — 3 failures, all fenced

- `glob_self_imports_never_grow_per_crate`: three files over a zero
  ceiling — `package-source` `tree/capture/traversal.rs`
  (BUILD-PACKAGES-GATE), `extents` `activation_claims/tests.rs`
  (RUNTIME-SIZED-ACTIVATION-STORAGE), `validation`
  `value_custody/expression_types/result_type.rs` (MATCH-SELECTIVE-LOWERING).
- `abstract_to_target_translation_validation_cannot_reenter_its_producer`:
  stale required-string pin — the test asserts
  `source.nodes.len() != block.operations.len() + 1` in
  `scalar_graph_input/target/control_flow.rs`, but `a5e89951766` ("t2s:
  legalize borrowed descriptor-parameter calls with replayed contract
  custody") re-expressed the count as
  `source.nodes.iter().filter(|node| !is_descriptor_declaration(node)).count()`
  `!= block.operations.len() + 1`. The parity check is intact; the pin
  needs the filtered form. `tests/architecture/layering.rs` is fenced to
  ARCH-LAYERING-REPLAY-PIN (lease to 22:53Z).
- `every_declared_custody_field_inventory_drives_a_substitution_matrix`:
  23 `*FieldForTest` inventories in `image-emission/tests/artifacts/
  installation_function_nested_custody.rs` plus one in
  `compilation-report/src/pcc/native_evidence/custody_tests.rs` declare
  inventories no `run_one_field_substitution_matrix`/`_for_test` hook
  drives — the family's driver legs are the open CUSTODY-MUTATION-COVERAGE
  claim (compilation-report side, lease to 05:23Z Sep 21).

## 4. check — green

`cargo check --workspace --all-targets` completes in 1m09s warm. The
`external-roots` E0432 (`effects::ComponentEraJournal{,Roster}` removed by
`2d8c5136cc` while `program_local` still imported it) that compile-blocked
gates 2/4/5 in the z142/z173 measurements is resolved at this head.

## 5. libtests — 16159/16170 (11 failed, 2 skipped, 1555s)

Down from 95 failures at `a9fa1a4fe6` and 95 at `96b4afed92` — the bulk of
the workspace went green as the sibling red-family repairs landed. The
eleven remaining failures split into four families:

- **package-manager × 6** — `operations::check_project::semantic`
  `retained_check_root_uses_final_consumer_bindings_and_requested_entry`,
  `compile_project::receiving_admission`
  `accepted_console_customer_receives_admission_only_under_a_sufficient_policy`,
  `review::candidate::compilation` `discovery_proposes_*` ×3 +
  `review_publishes_the_named_component_description_for_an_independent_
  selection`: the known `Service<R>`-only respell fixture family —
  fixtures still spell bare boundary traits in value position
  (`provider selection operand does not resolve to one visible product
  declaration` and kin). Same family recorded at the prior legs.
- **fixed-fuel count encoding × 3** — `checked-trees-to-lowered-psi`
  `structural_control_cases` `ranked_countdown_lowers_to_verified_resumable_
  interpreter_execution` + `ranked_u64_countdown_fails_closed_when_fixed_
  fuel_exceeds_u64` and `external-roots` `stack_and_fuel::fixed_fuel`
  `installed_natural_cycle_safe_point_catalog_binds_to_one_occurrence`:
  expectations pin fuel `3` where the code now reads `25769803776`
  (`3 << 33`) — a packed-field encoding landed without the fixture
  expectations being re-derived.
- **native-realization × 1** — `native_product::realization`
  `exclusion_taking_entry_reaches_mechanism_adjudication`: same
  `Service<R>` respell family ("field `sink` on data `Main` names bare
  boundary trait `Sink` in value position").
- **package-evidence × 1** — `capture::quotients`
  `total_direct_define_projects_one_deterministic_recoverable_review_row`:
  "ordinary checked lowering must not admit the proof-only request".

Notably green this leg: `selected-dispatch` (0 failures — was 64 at
`a9fa1a4fe6`), `terminal-codec` (was 20), `a2a2` (was 14), `s2as` (was 6),
`calling-conventions` (was 4), `t2c2` `composed_operand_catalogs` family,
and the two stale `check --workspace` fixtures that blocked gate 4 at
z142/z173.

## Verdict

`check` flips green; the remaining four commands stay red on sibling-fenced
surfaces (fmt: NAMED-TRAIT-OPERATORS; clippy: BUILD-PACKAGES-GATE;
arch-test: glob ratchet legs + ARCH-LAYERING-REPLAY-PIN +
CUSTODY-MUTATION-COVERAGE; libtests: the `Service<R>` respell family under
TWO-AXIS/FOREIGN-RETAINED-ARGUMENT-BACKING-area claims plus the new
fixed-fuel packing and package-evidence quotients regressions, which need
owner attribution on the board). Gate stays open.
