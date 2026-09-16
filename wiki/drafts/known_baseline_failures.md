# Known baseline failures

Attribution evidence for test failures that reproduce at a recorded revision
without a task diff. A worker that hits one of these may cite this table —
command, revision, and failure set — instead of re-running a stash baseline.
Refresh or remove a row when its failures are fixed or when a task's diff could
plausibly interact with them; a listed failure does not excuse an unexplained
failure in affected behavior, and this file is not validation policy (see
[AGENTS.md](../../AGENTS.md#validation-scope)).

Rows verified by independent stash-baseline reproduction at revision
`8220f55febc1` on 2026-09-13, macOS arm64, unless noted otherwise.

## typed-trees-to-checked-trees

`cargo nextest run --workspace --lib --no-fail-fast` at 2c234a684c on
2026-09-16 (macOS arm64) reports 14 failures in this crate's lib tests:
`execution::terminal_unit::calls::computation_arguments::tests::scalar_caller_retains_call_produced_record_local_before_getter`,
`tests::borrow::checks::persistent_storage::accepts_static_persistent_copy_across_attached_transparent_result_frame`,
`tests::contracts::boolean_call_results::call_produced_boolean_guarantees_reject_altered_call_and_argument_custody`,
`tests::flow::terminal_cleanup::machine_edges_and_projections::affine_locals_fail_closed_in_the_whole_parameter_edge_slice`,
`tests::flow::terminal_unit::calls::boundary_calls::static_boundary_reaches_keep_every_direct_intrinsic_and_parameter_call`,
`tests::flow::terminal_unit::cleanup::unit_and_scalar_cleanup::unit_body_affine_local_slice_fences_every_wider_local_shape`,
`tests::flow::terminal_unit::nested_boundary_results::nested_boundary_results_keep_dense_postorder_and_exact_temporary_transfers`,
`tests::flow::terminal_unit::nested_boundary_results::nested_ordinary_results_keep_postorder_and_exact_boundary_operand_roles`,
`tests::flow::terminal_unit::state_graph_scalars::general_state_graph_rejects_interleaved_scalar_storage_write`,
`tests::generics::symbolic_ranges::discarded_calls_in_open_templates_validate_inferred_const_bounds`,
`tests::multiplicity::borrowed_observations::indexed_operand_access_preserves_shared_collection_and_owned_index`,
`tests::multiplicity::obligations_and_state_call_results::consuming_call_that_returns_an_obligation_transfers_its_origin`,
`tests::termination::crash_routes::crash_fallthrough_and_equality::erased_record_equality_is_not_mistaken_for_empty_record_equality`, and
`tests::values::initializer_call_computations::later_results::direct_boundary_result_operands_retain_exact_nonself_transfer_events`.
The earlier 12-failure row (8220f55febc1, 2026-09-13) named a subset of these.
The crate's `src/tests`, `src/flow`, `src/values`, `src/facts`, and
termination/multiplicity check areas were under live work claims when this row
was refreshed, so no attribution beyond the names is recorded here.

## compiler build-target activation

`mbx nextest run -p compiler --test build_target_activation` — 49/51 pass; 2
fail:
`x86_feature_admission::source_fma_then_attached_unit_call_stays_inside_one_canonical_mxcsr_envelope`
and
`x86_feature_admission::terminal_product_retains_exact_fma_operation_plan_and_x86_admission`,
both with `native artifact native instruction selection failed: FMA provider
transport is not implemented in the common instruction pipeline`. x86 FMA
provider transport is unimplemented; the failure is not host-specific.


## package-evidence

`cargo nextest run -p package-evidence --no-fail-fast` at 34cc842d85 plus the
three fixture commits beside this row (2026-09-16, macOS arm64): 632 run,
631 passed, 1 failed:
`authority::toolchain_provenance::declared_hardware_service_reach_does_not_infer_physical_authority`
("domain `Extent::Granted` establishment route `ExtentRootProvider::grant`
does not resolve to one exact trait"). The fixture package declares its own
`pub boundary trait ExtentRootProvider {}` and imports nothing; since
5d134569b6 seeded the hosted entry contract and its `core` imports into
every hosted package-aware compilation, `core/extent.omg` is in the program
and `syntax-trees-to-symbol-resolved-trees/src/selection/signature_free_requirements.rs`
(`resolve_signature_free_requirement`) collects every same-named trait
program-wide, filtered only by resolution stratum, so the package's trait and
`core`'s collide as `TraitNotUnique`. Per wiki/spec/language/modules.md a
package's declarations are not in `core`'s scope, so the fixture is valid and
the resolver over-collects; the fix is scoping candidates to the occurrence's
module/dependency scope, whose predicate lives under the live
MODULE-NAMESPACE-RESOLUTION claim.

## native-differential `terminal_psi_source`

`cargo nextest run -p omega-native-differential-test --test terminal_psi_source
--no-fail-fast` at 76df1b15c7 plus 1a56e53b8d on 2026-09-16 (macOS arm64):
90 run, 82 passed, 8 failed, in three pre-existing families.

- Hosted-receiver custody (5): `control_flow_cleanup_source_reaches_the_publication_gate`,
  `retired_selected_lowering_rejects_before_native_publication`,
  `selected_preterminal_optimizers_rejoin_one_native_pipeline`,
  `selected_progress_free_source_stages_non_visible_terminal_candidate`,
  `selected_source_entry_retains_build_bound_progress_for_terminal_publication`
  fail with `native artifact ProgramEntry receiver provisioning failed: hosted
  receiver requires exact checked initialization and cleanup custody`
  (`native-realization/src/native_realization.rs`, since 96854008a0 and
  4f65a07840): the target's `stage_terminal_component_with_policies` helper
  builds the native request by hand and never supplies the checked entry the
  production route attaches through `with_checked_entry`. Repair is a harness
  migration onto the stage crate's route (**ENTRY-CONTENT-ROOTS** area).
- Frontend-drop custody ordering (2):
  `contracts_and_frontend_drop::terminal_production_requires_typed_custody_but_not_debug_presentation`
  and `locals_calls_and_short_circuit::checked_source_scalar_locals_become_terminal_block_values`
  expect `Unsupported("scalar source custody has no authored state")` but now
  reach the earlier attached-Unit parameter gate
  (`checked-trees-to-lowered-psi/src/unit/attached_unit/parameters.rs`,
  `carries_parameter_custody`) first: `Unsupported("direct Unit parameter plan
  has no exact typed machine")`. The expectation dates from 3fcf8240e3; the
  gate order moved in a 2026-09-15 lowering commit and was not bisected.
- `locals_calls_and_short_circuit::checked_source_staged_local_sequences_before_an_explicit_crash`
  (1): `UnsupportedControlFlow(MachineId(1))` from
  `abstract-operations-to-target-operations/src/lowering/control_flow.rs`;
  expectation from 2694d433d3, not bisected.

## native-differential `pipeline_ownership`

`cargo nextest run -p omega-native-differential-test --test pipeline_ownership
--no-fail-fast` at 7a63ba8cfd plus the lint, expectation, and retired-control
commits beside this row (2026-09-16, macOS arm64): 349 run, 344 passed, 5
failed. (The duplicate-import compile failure recorded earlier was removed
upstream in 9d4b45b0cf; the two u8 legalization negatives whose premise
a63284e305 retired now pin the admission instead.)

- Structural Unit fail-closed (5):
  `stages::realization::structural_units::leaf_object::structural_extent_unit_leaf_reaches_canonical_object_artifact`,
  `..::publication::claim_completion_prefixes_publish_as_metadata_without_instruction_spans`,
  `..::publication::installed_structural_provider_call_reaches_shared_publication`,
  `..::publication::structural_call_publication_preserves_owned_indirect_arguments`, and
  `..::structural_call::structural_unit_call_reaches_post_allocation_machine_custody`
  stop at `UnsupportedControlFlow(MachineId(..))` from
  `abstract-operations-to-target-operations/src/lowering/control_flow.rs`,
  the fail-closed behavior 8aac311045 documents for qualified structural
  calls, executable cleanup, installed-provider and descriptor cases that
  lack ordinary graph joins (**TRANSLATION-VALIDATION** area).

## checked-interpreter integration tests

`cargo nextest run -p checked-interpreter --test borrowed_subslices` at
c7465c23bc (macOS arm64): `inline_const_generic_selectors_execute_distinct_inferred_extents`
fails at checking with "machine `Main::endpoint` state `endpoint` terminal
expression returns a value not provably within its declared range" for a
`<const N: u64>` endpoint returning `u64 [0..=3]` from an inferred extent; a
generics checker gap (**STRUCTURAL-GENERIC-MATCHING** / **RUNTIME-VALUE-GENERICS**
areas, both under live claims when recorded).

## Host note (macOS)

`rust-objcopy` emits `dyld: Library not loaded: @rpath/libLLVM.dylib` (SIGABRT)
during test-binary linking on the pinned `nightly-2026-09-04` toolchain on
this host. It is a warning in build output only and does not fail compilation
or tests; do not investigate it as a test failure.

## Intel macOS host gap (x86_64-apple-darwin)

Environmental, not a test regression, verified across the macw3 wave on
2026-09-14 (independent sessions reproduced identical sets on unmodified
bases including `d1b7165cd6`, `07782416b4`, and `1055e88f31`):

- `TargetProfile::host()`
  (`omega-rust/omega/representations/target/src/lib.rs`, `host()`) has cfg
  arms for macos-aarch64, linux-aarch64, linux-x86_64, and windows-x86_64
  only, so host-profiled tests panic with `unsupported host profile for
  Omega native planning`. Observed sets: ~20 `package-manager` ops tests and
  2 `compiler::request` tests in the workspace `--lib` run.
- `native_hosted_target()` in `compiler/tests/canary_suite.rs` has the same
  four cfg arms, so `mbx nextest run -p compiler --test canary_suite` does
  not compile on this host.

On this host, route `omega` invocations through an explicit `--target` (for
example `linux_x86_64`) and report native-host coverage as unavailable rather
than re-running the baseline; a `MacosX64` host profile is open board work,
not a fix to inline into a task.
