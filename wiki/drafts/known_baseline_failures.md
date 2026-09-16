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

## checked-trees-to-lowered-psi

`cargo nextest run --workspace --lib --no-fail-fast` at 2c234a684c on
2026-09-16 (macOS arm64): 1 failure,
`tests::store_lowering::write_only_stores_and_subloans::finite_literal_index_suffix_crosses_source_codec_and_verification`
("deep literal-index admission is exclusive to write-only access"). The
crate's `src/tests` was under a live claim when recorded.

## compiler build-target activation

`mbx nextest run -p compiler --test build_target_activation` — 49/51 pass; 2
fail:
`x86_feature_admission::source_fma_then_attached_unit_call_stays_inside_one_canonical_mxcsr_envelope`
and
`x86_feature_admission::terminal_product_retains_exact_fma_operation_plan_and_x86_admission`,
both with `native artifact native instruction selection failed: FMA provider
transport is not implemented in the common instruction pipeline`. x86 FMA
provider transport is unimplemented; the failure is not host-specific.


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
