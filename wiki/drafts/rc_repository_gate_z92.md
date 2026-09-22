# RC-REPOSITORY-GATE — linux x86-64 leg (z92)

Record of the `RC-REPOSITORY` release gate block from
[rust_compiler_completion.md](rust_compiler_completion.md#release-matrix) run
on linux x86-64 (swarm VM, cargo — `mbx` unavailable) at worktree commit based
on `ff2f489bbff` (origin/main, 2026-09-20), plus this leg's unfenced repairs.
Prior legs: [rc_repository_gate_closure_z173.md](rc_repository_gate_closure_z173.md)
(`96b4afed92`), [rc_repository_gate_linux_x86_64.md](rc_repository_gate_linux_x86_64.md)
(`5053b420929`).

**Gate does not close: 3 of 6 commands stay red, every residual inside a live
sibling claim.** This leg repaired all unfenced lint drift reachable before the
first fenced error (6 sites across 4 files) plus the one unfenced fmt-drift
file. The compile break that blocked commands 2/4/5 at `96b4afed92` is gone:
`check --workspace --all-targets` completes clean at this tip.

| # | Command | Exit | Result |
| --- | --- | --- | --- |
| 1 | `cargo fmt --all -- --check` | 1 | residual is exactly one fenced file: `checked-interpreter/tests/trait_operators.rs` (NAMED-TRAIT-OPERATORS → 01:45Z). This leg formatted `omega/tests/package_commands/inspection.rs` |
| 2 | `cargo clippy --workspace --all-targets -- -D warnings` | 101 | 10 lib sites surfaced before dependents stop building: 5 fenced (§2), 5 repaired here; downstream test-target warnings enumerated in §2b |
| 3 | `cargo nextest run -p omega-architecture-test --all-targets --no-fail-fast` | 100 | 575 tests: 573 passed, 2 failed — glob ratchet (2 files, both fenced) + custody_mutation_matrix (§3) |
| 4 | `cargo nextest run -p compiler --test canary_suite --no-fail-fast --no-tests fail -E 'test(=surface_and_targets::retired_domain_when_surface_is_absent_from_authored_corpus)'` | 0 | 1 test, pass (5.5s) |
| 5 | `cargo check --workspace --all-targets` | 0 | clean at this tip; the `96b4afed92` external-roots E0432 break and the `a9fa1a4fe6` stale native-differential fixtures are all repaired (§5) |
| 6 | `cargo nextest run --workspace --lib --no-fail-fast` | 100 | 16185 tests: 16174 passed (48 slow), 11 failed, 2 skipped — all sibling-lane clusters (§6) |

## 0. Repairs landed by this leg

All are mechanical `clippy`/`fmt` conformance fixes — zero behavior change:

| File | Lint | Fix |
| --- | --- | --- |
| `isa-x86_64/src/selected_form_encoding.rs` | `unused_imports` | dropped `use crate::x86_64_physical_register_model`; `selected_form_encoding/tests.rs` now imports it from `crate::` directly |
| `checked-trees-to-lowered-psi/src/emission/expression_validation.rs:48` | `needless_return` | tail expression |
| `checked-trees-to-lowered-psi/.../composed_control/state_graph/emission/state.rs:534,598` | `unwrap_or_default` | `or_insert_with(Vec::new)` → `or_default()` |
| `typed-trees-to-checked-trees/src/checks/contracts/writes.rs:1241` | `unnecessary_to_owned` | `&segments.to_vec()` → `segments` |
| `typed-trees-to-checked-trees/src/execution/terminal_unit/calls/structural_arguments.rs:1086` | `question_mark` | `let-else return None` → `?` |
| `omega/tests/package_commands/inspection.rs` | fmt drift | `cargo fmt` reflow |

## 1. fmt

`cargo fmt --all -- --check` residual at `ff2f489bbff` was two files;
`inspection.rs` was unfenced and is repaired here. `trait_operators.rs` sits
under NAMED-TRAIT-OPERATORS's claim — left for its owner.

## 2. clippy — residual is fenced

After this leg's fixes, the remaining workspace lint sites before dependents
stop building:

- `syntax-trees-to-symbol-resolved-trees/src/preparation/generic_data/substitution.rs:274` — `clone_on_copy` (`membership.clone()` on a Copy type). Fenced by RUNTIME-VALUE-GENERICS → 05:47Z.
- `typed-trees-to-checked-trees/src/checks/termination/progress/origins.rs:72,858,1042` — `needless_borrows_for_generic_args`, `question_mark` ×2. Fenced by TPR6 → 06:19Z.
- `typed-trees-to-checked-trees/src/proof/mathematical_signature.rs:1377` — `question_mark`. Fenced by MATH-FOUNDATION-BINDINGS → 00:09Z.

### 2b. Downstream test-target warnings (already visible under `cargo check`, will error under `-D warnings` once the lib errors clear)

- `package-evidence` test suite: 9× `use crate::support` unused imports — BUILD-PACKAGES-GATE fence → 23:25Z.
- `package-manager` test suite: `package_capability_conflicts.rs:1` unused import — same lane.
- `compiler` `canary_suite`: `time_hosts_and_indexed_storage.rs:4` unused import + dead `SHARED_RECEIVER_PLAIN_FIELD_WRITE` const.
- `omega-native-differential-test` `terminal_psi_conditional.rs:199,202` — dead `ScratchDirectory` machinery.

## 3. architecture — 2 failures

- `glob_self_imports_never_grow_per_crate`: `extents` (1 file) + `validation` (1 file) over ceiling — the two crate dirs are fenced (RUNTIME-SIZED-ACTIVATION-STORAGE / MATCH-SELECTIVE-LOWERING lanes).
- `custody_mutation_matrix::every_declared_custody_field_inventory_drives_a_substitution_matrix`: 14 `*FieldForTest` inventories in `image-emission/tests/artifacts/installation_function_nested_custody.rs` and 1 in `compilation-report/src/pcc/native_evidence/custody_tests.rs` declare substitution field inventories with no driving matrix — sibling in-flight work (image-emission nested custody / RC-PCC-REPLAY-GATE lane).

## 4. canary row — green

`retired_domain_when_surface_is_absent_from_authored_corpus` passes (5.5s).

## 5. check — clean

`cargo check --workspace --all-targets` finishes (74s warm): only warnings.
Both fixtures the `a9fa1a4fe6` board measured red are repaired upstream
(`ordinary_graph_controls.rs` handles `Crash`; `decision_custody.rs` compiles),
and the `96b4afed92` `external-roots` E0432 is gone.

## 6. workspace lib — 11 failures, all sibling lanes

Full `--workspace --lib` run: 16185 tests, 16174 pass (48 slow), 11 fail,
2 skip. Failures:

- `package-manager` ×6 — `Service` carrier/domain diagnostics (`the core Service carrier is closed`, `no domain named Bound`) and review-row discovery drift — BUILD-PACKAGES-GATE / domain lanes.
- `checked-trees-to-lowered-psi` ×2 — `structural_control_cases::ranked_countdown_*` reproduce identically at unmodified `5958706064`: preexisting edge-rank packing drift (`25769803776` vs `3`), unrelated to this leg's mechanical rewrites.
- `external-roots` ×1 — `installed_natural_cycle_safe_point_catalog_binds_to_one_occurrence`.
- `native-realization` ×1 — `exclusion_taking_entry_reaches_mechanism_adjudication` (bare `Sink` boundary-trait spelling rejected — sibling service-carrier work).
- `package-evidence` ×1 — `capture::quotients` review row drift — BUILD-PACKAGES-GATE lane.
