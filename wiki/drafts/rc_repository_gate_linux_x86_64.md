# RC-REPOSITORY-GATE — linux x86-64 leg

Record of the `RC-REPOSITORY` release gate block from
[rust_compiler_completion.md](rust_compiler_completion.md#release-matrix) run
on linux x86-64 (swarm VM, cargo — `mbx` unavailable) at commit
`5053b420929` (origin/main, 2026-09-20). This leg re-measures the gate after
`a1daf35f2e`
([rc_repository_gate_closure_z142.md](rc_repository_gate_closure_z142.md)).

**Gate does not close: 3 of 6 commands are red at this head, and one of them
aborts the run at build time.** fmt is green again — the 59-hunk drift the
prior leg recorded is fully repaired.

| # | Command | Exit | Result |
| --- | --- | --- | --- |
| 1 | `cargo fmt --all -- --check` | 0 | clean — zero diffs at this head |
| 2 | `cargo clippy --workspace --all-targets -- -D warnings` | 101 | `clippy::permissions_set_readonly_false` unchanged (§2) plus the §4 compile failure |
| 3 | `cargo nextest run -p omega-architecture-test --all-targets --no-fail-fast` | 100 | 575 tests: 561 passed, 14 failed |
| 4 | `cargo nextest run -p compiler --test canary_suite --no-fail-fast --no-tests fail -E 'test(=surface_and_targets::retired_domain_when_surface_is_absent_from_authored_corpus)'` | 0 | 1 test, pass (3.5s) |
| 5 | `cargo check --workspace --all-targets` | 101 | `proof` lib test: 8 hard errors (§4); the earlier `external-roots` `ComponentEraJournalRoster` break is repaired |
| 6 | `cargo nextest run --workspace --lib --no-fail-fast` | 101 | build aborts on `proof` lib test — 0 tests executed |

## 1. fmt — clean

Zero diffs. The 16 claim-fenced files the prior leg left plus the 8 it
formatted are all tidy at this head; the baseline-green fmt lane and the
sibling file owners have landed their legs.

## 2. clippy — unchanged lint plus compile break

`packages/sources/acquisition/src/tree/capture/traversal.rs:513`
`writable.set_readonly(false)` — `permissions_set_readonly_false`, in the
`package-source` lib test. Unchanged since `f1e9a3733d`; the file is fenced
to BUILD-PACKAGES-GATE. The `proof` lib-test compile failure (§4) also
surfaces under clippy and blocks any deeper lint sweep.

## 3. architecture — 14 failures

Four clusters at this head:

- `layering
  abstract_to_target_translation_validation_cannot_reenter_its_producer`
  — "independent graph replay must retain source.nodes.len() !=
  block.operations.len() + 1" (layering.rs:4328). New family.
- `glob_self_imports glob_self_imports_never_grow_per_crate` — ratchet grew:
  acquisition 1, selected-instructions-to-selected-instructions 2, extents 1,
  proof-admission 1, validation 1 (all ceilings 0).
- `validation_integration` — 11 rows, all failing on the same diagnostic:
  `return type of signature SchedulerAdmission.grant names bare boundary
  trait SchedulerRuntime in value position; the intrinsic Service<R> carrier
  is the only service value spelling`. The ENTRY-CONTENT-ROOTS exact
  `Service<R>` cut rejects the bare-trait fixtures; the recast-witness and
  provider-receiver suite needs fixture migration or the admission spelled
  the new way.
- `custody_mutation_matrix
  every_declared_custody_field_inventory_drives_a_substitution_matrix` —
  23 declared `*FieldForTest` substitution inventories with no driving
  `*_for_test` hook / `run_one_field_substitution_matrix`: 22 in
  `image-emission/tests/artifacts/installation_function_nested_custody.rs`,
  1 in `compilation-report/src/pcc/native_evidence/custody_tests.rs`
  (`NativePlacedImageEvidenceFieldForTest`). Inventories declared, matrix
  legs not yet landed.

## 4. check — `proof` lib test compile failure

`omega-rust/psi/semantics/proof/src/checker/measurement.rs:370–385` (lib
test module): 4× E0422 `ProofNode` + 4× E0433 `ProofRule` — the test
constructs `ProofNode { .. }` / `ProofRule::Assumption` without importing
them; rustc suggests `crate::checker::measurement::{ProofNode, ProofRule}`
or `proof_admission::{ProofNode, ProofRule}` — a rename/move mid-flight.
`--all-targets` builds lib tests, so the command fails; plain
`cargo check --workspace` is unaffected. The `external-roots`
`ComponentEraJournalRoster` unresolved import present at `6d1fa4caa4c` is
repaired at this head.

## 5. workspace lib run — aborts at build

`cargo test --no-run --workspace --lib` exits 101 on the `proof` lib-test
errors above; cargo fails the build plan and nextest executes 0 tests. The
prior leg's 88-failure clusters cannot be re-measured until the `proof`
lib test compiles again — the gate row needs a re-run after that lands.

## Delta vs the prior leg (`a1daf35f2e`)

- fmt: 59 hunks / 24 files → clean (repaired).
- check: `pipeline_ownership` E0308/E0004 harness drift → repaired; new
  `proof` lib-test break in its place.
- workspace lib: 88 failed / 15,846 passed → unmeasurable (build abort).
- clippy lint and the recast-witness architecture family: unchanged.
