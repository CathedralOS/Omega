# Known baseline failures

Attribution evidence for test failures that reproduce at a recorded revision
without a task diff. A worker that hits one of these may cite this table —
command, revision, and failure set — instead of re-running a stash baseline.
Refresh or remove a row when its failures are fixed or when a task's diff could
plausibly interact with them; a listed failure does not excuse an unexplained
failure in affected behavior, and this file is not validation policy (see
[tools/testing.md](../../../tools/testing.md#validation-scope)). Delete this draft when no row
survives — every listed failure fixed or re-attributed to a task diff.

## Drain pass at `b63bc68fabc` (2026-09-27, macOS arm64)

Every row recorded before this date was re-run or re-verified at that revision.
The 2026-09-25 consolidation removed most harnesses this ledger measured:

- `354622565c1` deleted the architecture-enforcement suite — the
  `omega-architecture-test` row (the `scoped_lookup_maps` merged-range red)
  is unmeasurable; the check itself no longer exists.
- `c60792d7bdd` made the corpus gate the compiler's only fixture runner —
  the `canary_suite` pass/fail-canary rows, the "pass fixtures that no
  roster runs" census (rosters are gone; `corpus_runner` compiles every
  fixture directory, so unexercised fixtures are impossible), the
  `build_target_activation` FMA row, the `package_compilation_inputs` row,
  and the canary `_runs` native-execution observation are all superseded.
  The `pass/` and `fail/` corpus rows below carry what remains live. The
  FMA transport capability gap itself stays on the board under
  **FMA-PROVIDER-PIPELINE-TRANSPORT**.
- `c74472a07a1` removed the `omega-native-differential-test` crate — the
  RC-NATIVE-MATRIX first reading, `terminal_psi_source`, compile-floor and
  `pipeline_ownership` rows are unmeasurable. Their live conditions moved
  to the corpus gate, and the recorded failure families they contained
  (hosted-receiver custody, structural-unit lowering, `Service<R>`
  spellings) all measure green through the corpus or crate suites at this
  revision.
- `612e3156b80` folded the six orchestration crates into one `compiler`
  crate — the `native-realization` `--lib` member
  (`exclusion_taking_entry_reaches_mechanism_adjudication`) is gone with
  its package, and `compilation-report`'s `pcc::native_evidence` member is
  likewise gone.

Rows verified **closed** at this revision (each named member re-run and
passing on this host):

- `terminal-verifier`: all recorded members — the four
  `nominal_affine_cleanup` rejection-loss members (now renamed but pinning
  the same rejections), `trusted_surface::recorded_digests_match_the_working_tree`
  and `source_coverage_fires_on_a_changed_implementation`,
  `jump_edge_residual_discards_close_the_projected_argument_root_in_order`,
  and `direct_field_partial_affine_return_rejects_forged_conservation_shapes`
  — 57/57 pass. The trusted-surface digests now match the working tree; the
  deliberate red is resolved, so this row no longer shields
  **PCC-CANONICAL-SEMANTIC-LEDGER** evidence.
- `terminal-interpreter` `affine_cleanups::*`: 25/25 pass — the
  **OWNED-SELF-RECEIVER-AFFINE-DISCARD** witnesses are green.
- `optimization-unit-semantics`: 123/123 pass — the three
  **CLOSED-SUM-EDGE-DISCARD-EVIDENCE-DISAGREEMENT** members are green.
- `external-roots`
  `stack_and_fuel::fixed_fuel::tests::installed_natural_cycle_safe_point_catalog_binds_to_one_occurrence`:
  pass — the ranked component-scale segment-bound family is closed.
- `package-manager` --lib: all six carried members pass
  (`retained_check_root_uses_final_consumer_bindings_and_requested_entry`,
  `accepted_console_customer_receives_admission_only_under_a_sufficient_policy`,
  the three `discovery_proposes_*` members, and
  `review_publishes_the_named_component_description_for_an_independent_selection`).
- `typed-trees-to-checked-trees` --lib: 1697/1697 pass — the section stays
  closed.
- `checked-trees-to-lowered-psi`: every recorded residual member passes —
  the 18 missing-transitive-machine-plan members
  (`provider_attachment_source` ×6, `unit_state_graph::provider_attachments`
  ×9, `unit_plan_omissions` ×3, `retention::conformance_applications` ×3,
  `tests::composed_operand_catalogs` ×5,
  `composed_caller_lowers_a_retained_scalar_graph_call_result`), the
  scalar-return custody member
  (`effectful_discarded_call_writes_before_return_across_fuel`), the four
  `scalar_array_source::cyclic` index-panic members, the
  `unranked_self_bindings_validate_without_claiming_finite_fuel` verdict,
  the `structural_control_cases::ranked_*` pair, and the
  `reference_result_source` members (the recorded
  `projected_record_argument_preserves_original_storage` is gone with the
  consolidation; `projected_record_argument_rejects_changed_leaf_custody`
  and its siblings all pass) — 107/107 filtered members pass. The pathological proof-search member
  `nominal_affine_source::integer_comparison::mixed_nominal_integer_comparison_converges_before_one_shared_cleanup_return`
  **terminates in 16 s** — the >1400 s blowup is gone.
- `terminal-codec`: the four macOS-only members
  (`sections::structural_block_wire_tests::*`,
  `sections::trust_graph::*`) — 17/17 filtered members pass.
- `abstract-operations-to-target-operations` --lib: 95/95 pass — the four
  `structural_borrows`/`structural_calls` members and the
  `bd5648555c`-attributed pair are green.
- `checked-interpreter` suite:
  `borrowed_subslices::inline_const_generic_selectors_execute_distinct_inferred_extents`
  passes under the consolidated `--test suite` target (the per-file
  `borrowed_subslices` target is gone, `autotests = false`).

## corpus gate — pass tier

`python3 tools/corpus_gate.py --filter pass/` at `b63bc68fabc` (2026-09-27,
macOS arm64): 721 pass fixtures ran — 472 `checked`, 249 `rejected`, 0
rejected without their expected fragment. Against the checked-in golden:
51 rows moved — 9 moved `rejected` → `checked`
(`arithmetic/saturating_signed_divide_min_by_neg_one`,
`blockexec/blocking_executor_custody_claims_compile`,
`build/runtime_depend_mapping_exit`,
`build/runtime_main_source_builder_is_ordinary_exit`,
`capabilities/uses_caller_folder`,
`dependent/runtime_dependent_subtract_exit`,
`structs/uninit_record_local_exit`,
`structs/uninit_record_member_store`,
`traits/equatable_record_equality_exit`), 36 moved diagnostics while
staying in their recorded family, and 6 fixtures are new since the golden
(`capabilities/callback_direct_install_compile` rejects;
`ownership/linear_self_receiver_disposition`,
`structs/copied_array_field_shared_record`,
`structs/mutable_array_record_state_transfer`,
`structs/mutable_record_state_transfer`,
`structs/nested_unused_provider_fields` check). The golden
(`tests/omega/corpus_outcomes.txt`) needs a re-record on a measured
revision; **CANARY-CORPUS-ROSTER-ACCOUNTING** owns the golden — do not
re-pin it from an in-flight worktree.

The 249 rejected pass fixtures attribute to these families (golden
diagnostics, confirmed same-status by the run above; several phase strings
moved in-window but the families are stable):

- 154 `selected ProgramEntry establishment rejoins 0 Terminal attachment
  identities; expected one` — the missing transitive checked Unit plan,
  led by `statement sequence: local data: structural call binding` (26),
  `state graph: terminator: conditional successors: guard expression`
  (10), `structural field store` variants (~35), `state graph: terminator`
  variants (~20), `call operation` variants (~15), `attached data shape`
  (5), plus "unavailable callee" chains. Owners: **GENERAL-CYCLIC-EXECUTION**,
  **ENTRY-CONTENT-ROOTS**, **STATE-LOCAL-VALUE-FRONTIER**,
  **CANARY-CORPUS**.
- ~41 `native-artifact Terminal production failed: Lowering(...)` tails —
  `Unsupported` ×17, `InvalidTerminalModule` ×9, `InvalidUnitMachinePlan`
  ×6, `OperationProofUnavailable` ×5, plus singletons. Backend/lowering
  lanes; the unit-plan members share the family above.
- 8 `terminal-authority policy ... does not classify normalized foreign
  mechanism` — cross-target syscall/foreign classification; host-bound on
  this host.
- 7 `selected ProgramEntry Binding field ... requires a selected Fused
  provider` — provider selection (FilesystemHost, Clock/TimeHost, Console,
  Gui spellings); provider-selection lane.
- 7 canonical-optimization initial-validation rejects
  (`StructuralCatalogMismatch`, `StructuralCallContractMismatch`).
- ~10 checker singletons/small sets: implicit `Trapping`/`Bounded`
  weakening requires an explicit `as` (4), `Float::equal` fixed-token
  match-equality position (3, same family as the package-evidence reds
  below — **MATCH-SELECTIVE-LOWERING**), `cannot prove default-domain
  field requirement` (3), provisioned all-zero `Main` receiver (4),
  value-call-argument-is-a-machine (4), unimplemented boundary-operator
  applications (5: `F64::from_i32`, `F32::minimum`, `F32::negate`,
  `I8::from_f32`, `F32::from_f64` — **OPERATOR-BOUNDARY-CALLS**), and
  assorted one-member diagnostics. None is unexplained.

`--filter fail/` at the same revision: 441/441 fail fixtures reject, and
the only golden move is `providers/checked_boundary_operator_const_application_unselected`
now satisfying its expected fragment (`expected_satisfied` False → True).
The corpus gate enforces `expected.txt` fragments directly — the retired
fail-canary harness's fragment coverage is subsumed, and no fail fixture
fails to reject or rejects without its fragment.

Native leg: `corpus_gate --native` diffs against
`tests/omega/corpus_native_macos_arm64.txt`, seeded with four fixtures at
`5f8cad64f63` (`built`, `exit:70` each — native emission and execution are
sound on this host; the blocker set is compile-stage). Full-host native
coverage belongs to **RC-NATIVE-MATRIX-MACOS-ARM64**.

## package-evidence

`mbx nextest run -p package-evidence --no-fail-fast` at `b63bc68fabc`
(2026-09-27, macOS arm64): 568 run, 564 passed, 4 failed. Every member the
prior rows recorded — the 46-member `7b224763615` census, the
`capture::quotients` QUOTIENT-THEOREM-LIFT singleton, and both
unattributed `fast-suite sweep` members
(`review_projects_unselected_type_and_lifetime_generic_top_level_external_supply`,
`public_trait_shape_retains_boundary_parent_and_alpha_normalized_requirements`)
— now passes. The four live failures are new since that reading:

- `boundary_application_policy::float_match_applications_retain_selected_intrinsic_review`,
  `float_match_review_rejects_substituted_arm`, and
  `float_match_review_rejects_lost_enclosing_source_extent` — the
  FLOAT_MATCH fixture's `match identity(value) { identity(first) -> ... }`
  arms bind `Float::equal`'s fixed `==` token in a match-equality
  position, refused at `token_bound_machine_calls.rs` ("a position whose
  body supply is not implemented; only binary and indexed expression uses
  bind the declaration's own body"). Match-pattern equality is
  **MATCH-SELECTIVE-LOWERING**'s authored scope; whether the fixture
  should respell or the position gains supply is that lane's call.
- `operators::fixed_token_checked_adapters::boundary_machine_spelling_reviews_identically_to_the_operator_spelling`
  — "reviewed callable `CheckedMathProvider::subtract_impl` realizes
  top-level requirement `CheckedMath::subtract` without external
  executable supply not yet represented by package review" — a
  package-review representation gap on the machine spelling of a fixed
  token; **OPERATOR-BOUNDARY-CALLS** owns callable review rows (its claim
  is live).

## Host note (Linux x86-64, 2026-09-20)

- `cargo fmt --all` is not clean at c2ccb2a202: it rewrites ~18 unrelated
  files (external-roots interrupt table, module_machine_indices,
  multiplicity/termination checks, validation tests, ...). Treat
  repo-wide fmt output as drift, not signal; `cargo fmt --check` on touched
  files only is the scoped gate.
- `tools/verify.py` requires `tomllib` (Python 3.11+); this host runs
  3.10. Equivalent raw `omega --check --build-dir <dir> <app>/main.omg`
  and `omega run --keep <app>/main.omg` invocations cover the same legs.

## Host note (macOS)

`rust-objcopy` emits `dyld: Library not loaded: @rpath/libLLVM.dylib` (SIGABRT)
during test-binary linking on the pinned `nightly-2026-09-04` toolchain on
this host. It is a warning in build output only and does not fail compilation
or tests; do not investigate it as a test failure.

## Intel macOS host gap (x86_64-apple-darwin)

Closed as recorded: `TargetProfile::host_if_supported`
(`omega-rust/omega/backend/target/src/target_profile.rs:344`) now carries a
`macos + x86_64` arm returning `MacosX64`, so `host()` no longer panics with
"unsupported host profile", and `canary_suite.rs` — the other recorded
blocker — was deleted in `c60792d7bdd`. The ~20 `package-manager` and 2
`compiler::request` test names from the 2026-09-14 observation were not
re-measured (no x86_64-apple-darwin host in this drain), but the panic
mechanism is gone; end-to-end `MacosX64` functionality remains ordinary
board work, not a baseline row.
