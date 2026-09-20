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

`cargo nextest run -p typed-trees-to-checked-trees --lib --no-fail-fast` at
660f5af762 (2026-09-18, macOS arm64): 4159 run, 4153 passed, 6 failed. Three
are the long-standing set described below. The other three,
`tests::termination::rank_ranges::{computed_field_limits::
field_endpoint_formation_never_uses_final_cancellation_to_excuse_overflow,
field_coordinates::field_endpoints_require_defined_intermediates_and_exact_owned_carriers,
field_endpoint_arithmetic::constant_rank_endpoints_preserve_landing_and_rational_meaning}`,
appeared under the live **TERMINATION-RANKING-CHECKS** claim and belong to
that lane. The earlier reading at 30f4189a58 was 3991 run, 3988 passed,
3 failed. The 13-failure row recorded at
2c234a684c was worked through test by test; ten were stale fixtures or retired
premises (each commit names the introducing revision and the rule that decided
it), and the two renamed tests are now
`static_boundary_reaches_keep_every_direct_intrinsic_and_requirement_call` and
`general_state_graph_retains_interleaved_scalar_storage_write`. The remaining
three, with the production site each needs:

- `tests::multiplicity::borrowed_observations::indexed_operand_access_preserves_shared_collection_and_owned_index`:
  since f1f9f898e2 `build_operator_facts` no longer re-seeds `NestedExpression`
  value rows, which had resolved the `[]` occurrence with wildcard operands;
  under exact typing the `self.buffer: Buffer` place does not match the
  declared `items: &Buffer` operand because
  `typed-trees/src/typed_trees/declarations/operator/indexing.rs::shared_collection_elements`
  adapts only slice shells (a `buffer: &Buffer` parameter operand passes; a
  `Buffer` place fails in every spelling). The settled
  [indexing receiver rule](../spec/language/expressions.md#indexing-and-ranges)
  uses ordinary attached-receiver borrowing, not a new auto-borrow rule for
  ordinary first parameters. OPERATOR-MACHINE-SUPPLY owns expressing this
  fixture with an attached receiver, retaining a separate explicit-parameter
  control, and checking the real loan/custody route. This recorded failure is
  not closed by the documentation settlement or by restoring wildcard matching.
- Repaired: `tests::multiplicity::obligations_and_state_call_results::consuming_call_that_returns_an_obligation_transfers_its_origin`
  was a checker regression, not a stale pin. Since 2dc27270bc
  `validation/src/value_custody/permission_provenance.rs::static_namespace_receiver`
  returned `Err("namespace differs from its selected nonself state")` when the
  callee takes `self` and is spelled through its data namespace
  (`Receipt::forward(issued)`), so provenance fell back to a fresh
  Statement-1 establishment for the result while `claim_identity` kept the
  Statement-0 origin. A bare data-namespace receiver is a non-operand
  regardless of whether the selected state takes `self` — the explicit `self`
  argument supplies the common origin — so the `is_self` rejection is gone
  and the sibling pin in `permission_provenance/tests.rs` ("a self formal
  cannot be replaced with a namespace") now asserts the namespace is omitted
  for the self-target spelling as well.
- Repaired: `execution::terminal_unit::calls::computation_arguments::tests::scalar_caller_retains_call_produced_record_local_before_getter`
  was a broken fixture: its inline source spelled `Region { base, ... }`,
  but struct literals require explicit `field: value` pairs; the fixture now
  spells `base: base`.

## terminal-verifier

`cargo nextest run -p terminal-verifier --no-fail-fast` at a66852a558
(2026-09-18, macOS arm64, dependency crates rebuilt from the same tree):
726 run, 719 passed, 7 failed, then 721 passed and 5 failed after the
jump-edge repair, and 732 run, 729 passed, 3 failed after the byte-field
repair beside this row. Only two of the six non-ledger failures were
`InvalidPartialAffineCleanup`; each of the others had its own cause.

- `trusted_surface::recorded_digests_match_the_working_tree` (and its
  `source_coverage_fires_on_a_changed_implementation` sibling): the ledger
  in `trusted_surface/sites.rs` records digests for twelve implementations
  (`semantic-vocabulary/src/content.rs` and `proposition`, terminal-psi
  proof-bundle admission and nodes, proof-admission `integer_rules/*`,
  `kernel.rs`, `lib.rs`, `evidence.rs`) that have since changed; the ledger
  needs re-recording by the lane that changed them, after review. It also
  reports `proof-admission/src/mathematical_core/tests/strict_layer.rs` as
  an unregistered source file under a trusted root.
- Repaired: `unranked_bindings::cyclic_scalar_targets_and_reachability_are_checked_before_dominance`
  and `unranked_views::every_cyclic_view_jump_checks_exact_arity` saw
  `InvalidPartialAffineCleanup` in place of `UnknownTargetBlock(BlockId(99))`
  and `StructuralJumpArityMismatch { edge: EdgeId(1), expected: 1, actual: 0 }`.
  The Jump-edge lane of `validation/affine_cleanup/continuation.rs`, added by
  517e86d465, resolved the terminator's target block and compared successor
  argument and parameter counts itself, in a pass that runs before
  `control_flow::validate_control_flow` and the structural frontier own
  those two checks. That lane now defers both shapes.
- Repaired: `structural_unit::boundary_buffers::ordinary_unit_byte_subloan_rejects_wrong_leaf_type_access_and_path`
  (mutation 1) and
  `structural_unit::boundary_buffers::boundary_buffer_rejects_wrong_leaf_erasure_access_and_type`
  were a lost rejection, not an ordering change: `canonical_leaf_shape`
  answered `Some` for `ByteSequence(carrier)` since 2fc3f6ad67, so a record
  field typed `ByteSequence(BorrowedView)` resolved by shape equality to the
  module's standalone declaration and satisfied a callee parameter directly,
  bypassing the inline presentation route that
  [byte views](../spec/terminal-psi/byte_views.md) and
  [structural access](../spec/terminal-psi/structural_access.md) make the
  only way a field-projected byte argument reaches a parameter. It answers
  `None` now, which closes the same hole in terminal-codec's independent
  validation; all seven leaf-shape consumers were followed and none needed
  explicit byte handling.
- Repaired: `structural_unit::boundary_buffers::fixed_array_views::fixed_byte_array_unit_view_keeps_existing_zero_array_admission_fence`
  expected `InvalidStructuralArrayLength(StructuralTypeId(3))` for a
  zero-length fixed byte array and saw
  `StructuralArgumentTypeMismatch { operation: OperationId(1), argument_index: 0, expected: StructuralTypeId(1), actual: StructuralTypeId(3) }`.
  The type-table fence in `validation/foundation/structural_types.rs` fires
  only when `terminal_semantics::scalar_array_leaf_shape` is `None`, which a
  byte array's primitive-scalar element is not, so the rejection had moved to
  the presentation check. The settled semantics admit a zero-length fixed
  array whose element carries a scalar leaf as an empty scalar array, while
  the `InvalidStructuralArrayLength` fence remains for elements without a
  scalar leaf: d96a0fda39 retargeted the fixture's element at a record so the
  pin still exercises the type-table fence on both the machine and boundary
  routes. The fence question this row left open is answered — presentation
  admits the scalar-leaf case, the type table owns the remaining rejection.
- `structural_scalar_fields::owned_reads::block_parameters::owned_successors_reject_same_arity_aliases_and_transfer_after_disposal`
  expects `InvalidStructuralSuccessorArgument { edge: EdgeId(3), place: PlaceId(2) }`
  and now sees `EdgeAffineDiscardsInvalid { edge: EdgeId(3) }`. Both reject
  the same module. `frontier/block_parameters.rs` documents the intended
  order — "phase one consumes each owned source before the residual and
  trivial cleanup for the same edge runs" — and the edge's trivial-discard
  roster is now checked first, so the reported error names the cleanup
  roster rather than the transfer of a disposed place.

## compiler canary suite (pass canaries)

`cargo nextest run -p compiler --test canary_suite -E
'test(pass_canaries_compile) | test(fail_canaries_reject_with_expected_diagnostic_fragment)
| test(/registered_/) | test(checked_only_canaries_are_not_backend_umbrella_members)'`
at 942f23e6f4 plus the climbing-sum repair beside this row (2026-09-18,
macOS arm64, 1157 s): 6 run, 5 passed, 1 failed. Only
`pass_canaries_compile` fails, on 22 registered fixtures, and this is the
freshly measured distribution **CANARY-CORPUS** asks for rather than the
older reading:

- 19 report `Lowering(InvalidUnitMachinePlan { .. })` from native-artifact
  Terminal production. The six `core/numeric_*` rows are
  **ARITHMETIC-POLICY-REALIZATION**, not a control-builder gap; the rest are
  the missing transitive Unit plan class (**GENERAL-CYCLIC-EXECUTION** and
  the state-graph route). The families:
  `core/numeric_*` conversion surfaces (6), `float/float_trapping_*` (5),
  `expressions/arithmetic_domain_trapping_*` (3),
  `control_flow/runtime_*_literal_dispatch_exit` (2),
  `capabilities/acquires_through_helper_return`,
  `host/runtime_gui_foreground_window_exit`, and
  `wire/runtime_wire_exact_array_without_count_exit`.
- `calls/statement_call_recursive_argument_compile`: "duplicate named
  machine overload `add`", with sibling reports of a duplicate `Nat` data
  declaration and a public interface selecting private `Nat`.
- `operators/runtime_integer_division_value`: "native-artifact production
  requires one exact selected program entry".
- `atomics/atomic_field_declared`: "macOS hosted receiver bridge lost exact
  contract, storage, or entry custody" (**ENTRY-CONTENT-ROOTS**; migrate the bare
  receiver field to the [intrinsically established service carrier](../spec/build/component_publication.md#service-bindings-and-era-entry)).

The `filesystem/native_*` family was reconstructed on 2026-09-18 at
40e22e0ce9. Its 54 fixtures beside `native_close` carried no `build.omg` and
imported the retired `omega::language::std` spelling, so
`OMEGA_PASS_CANARY_FILTER=filesystem/native_` reported "matched no active
pass canaries" rather than a failure: the family was inventoried through
`fixture_rosters/native_filesystem_canaries.rs` but sat on no executing pass
roster. With an ordinary build declaration and the `omega_language_std`
alias, and with the core library's unsigned widening machines publishing
their source carrier's range at 96c08b0109, and with the eight `struct stat`
byte-assembly fixtures re-spelled at 285da57703, 53 reach checked semantics
and are registered in `CHECKED_ONLY_PASS_CANARIES`. The one left
unregistered is `native_wrapper_write_all_result`, on
**MATCH-SELECTIVE-LOWERING**'s value-dispatch pattern limit. The eight that
assembled a field with `widen_u8_to_i64(byte) << 56`, and in three cases a
32-bit field with `widen_u8_to_i32(byte) << 24`, were unsound as authored:
those intermediates reach about 1.84e19 and 4.28e9 against `i64` and `i32`
ceilings of about 9.22e18 and 2.15e9, which no return range can discharge.
Each now assembles in the unsigned carrier of the field's own width, where
every shifted byte is representable, and reinterprets once at the landing.

`tests/omega/pass/filesystem/windows_set_file_time_exit` carries the same
`widen_u8_to_i64(byte) << 56` idiom and was not repaired: its canary is
Windows-gated, so neither its failure nor its repair can be measured on this
host. The obligation is target-independent, so it should refuse the same way
there, and the same unsigned-carrier re-spelling should close it.

`proofs/proof_inductive_climbing_sum` left this set when its accumulator
was bounded; the other four tests in the command pass, so the roster,
fail-canary fragments, and umbrella membership are all consistent.

## checked-trees-to-lowered-psi

Repaired: `unit_scalar_result_source::boundary_wrappers::ordered_boolean_guarantees::ordered_boolean_call_computations_preserve_normal_guarantees`
previously failed with `OperationProofUnavailable(ObligationId(9223372036854775809))`
at `tests/unit_scalar_result_source/boundary_wrappers.rs:41` (recorded at
00d0f9c15f on 2026-09-16 and at 51f21bb168, reconstruction inside
`src/proofs/operation_proofs.rs`). At d8d48fe4ff it passes in 13.8s under
nextest, so the operation-proof obligation on the `machine_calls` path is
discharged by the borrow-proof landings since.

`cargo nextest run -p checked-trees-to-lowered-psi --no-fail-fast` at
d8d48fe4ff (2026-09-20, Linux x86-64) runs the whole crate: 2143 run, 2085
passed, 58 failed (57 FAIL plus one test killed by SIGTERM after ~1300s).
The prior whole-crate reading at 9d0d864656 (2026-09-18, macOS arm64) was
2032 run / 20 failed; of its named groups, boundary byte buffers,
crash-member byte entries, and the ordered-boolean row are green now, while
scalar-return custody, provider attachment, and the attached-unit
borrowed-self case continue (the last under a new diagnostic — see the
Service<R> family). The current failure set attributes to six families:

- Stale bare boundary-trait fixture spelling (33 tests). All 30 failing
  library `tests::*` cases (`attached_unit_cases`, `composed_operand_catalogs`,
  `composed_unit_nested_control`, `dynamic_composed_unit`,
  `indexed_primitive_storage`, `structural_control_cases`) plus
  `tests/unit_plan_omissions.rs` ×3 panic at `src/tests.rs:82` /
  `tests/unit_plan_omissions.rs:16` on the same source check:
  ``field `console`/`runtime`/`output` on data `Main`/`Carrier` names
  bare boundary trait `Console`/`Output`/`TaskRuntime` in value
  position; the intrinsic `Service<R>` carrier is the only service value
  spelling``. The check in
  `typed-trees-to-checked-trees/src/checking/program_validation.rs` landed
  in 32f5182254 (2026-09-20) with fixture migration in the same-day
  0e1977994b; these fixtures still spell `console: Console` in value
  position. This is the ENTRY-CONTENT-ROOTS residual recorded on the board:
  unmigrated raw fixtures migrate to `&'s mut <boundary trait>` receivers or
  get `service.omg` injected, and fixtures that need service-activation
  semantics stay red until the receiver-lifecycle leg lands. Fixture fences:
  `src/tests` is under the PROOF-CERTIFICATION-BRIDGE claim and
  `checked-trees-to-lowered-psi/tests` under WRITE-ONLY-BORROW's
  integer-entry-ranges claim.
- Missing checked transitive machine plan (16 tests).
  `provider_attachment_source` ×6 stop at `signature` and
  `unit_state_graph::provider_attachments` ×9 plus
  `guarded_scalar_returns_source::stored_returned_cases_support_borrowed_refined_getters`
  stop at `state graph: state signature: parameter signature: attached data
  shape, state 0`, all surfacing as `InvalidUnitMachinePlan` "attached Unit
  closure is missing a checked transitive machine plan" / `` `X` has no
  admitted body (local construction stopped at <phase>) ``. The `signature`
  phase site is `execution/unit/control/checked_machine.rs` and the
  attached-data-shape guard is `execution/unit/calls/signatures.rs`, both in
  typed-trees-to-checked-trees unit construction; the fixtures pass source
  checking (already migrated in 0e1977994b) and stop while admitting the
  attached closure's bodies. Fences: `execution/unit/{control,state_graph,composed_control}`
  is under GENERAL-CYCLIC-EXECUTION and `execution/unit/{mod.rs,candidate_closure,calls}`
  plus `checked-trees-to-lowered-psi/src/unit` under UEFI-OS-HANDOFF. This is
  the continuing "provider attachment and results" group from the 9d0d864656
  reading.
- Crash predicate outside the selected scalar namespace (3 tests).
  `exact_affine_sibling_source::landed_affine_sibling_custody_crosses_source_codec_and_independent_verification`,
  `exact_shift_left_certificate_source::bounded_exact_left_shift_uses_only_its_canonical_certificate`,
  and `mixed_shift_source::erased_arithmetic_prefix_still_requires_its_own_certificate`
  each fail lowering with ``Unsupported("crash predicate value position is
  outside the selected scalar namespace")`` from
  `src/proofs/crash_routes/scalar_terms.rs`. On the `crash.site_guard` path
  (`scalar_graph/scalar_graph_module/state_emission.rs`,
  `lower_checked_crash_predicates(&crash.site_guard, self.parameters)`)
  predicate `Parameter`/`Local` positions index past the lowered `values`
  roster and that path supplies no erased roster
  (`checked_boolean_scalar_term(expression, values, &[])`), so positions
  moved into the proof-only erased lane by the erased-formal term work
  (294b6cfbf4, 2026-09-19) reject — consistent with the PROOF-RELEVANCE
  item's erased-binding namespace rule, not bisected; the identity-less
  crash-route discharge change ebef2636d8 (2026-09-20) is the adjacent
  suspect. `src/scalar_graph` is under the WRITE-ONLY-BORROW
  integer-entry-ranges claim.
- Scalar-return custody (4 tests, `tests/owned_record_return_source.rs`).
  `discarded_scalar_invocation_precedes_whole_owned_return`: the checked
  `facts.flow.terminal_unit_effects.for_machine` returns `None` for the
  record-returning `retain` after `_ = identity(mask); record` — no
  unit-effects plan is catalogued. `effectful_discarded_call_writes_before_return_across_fuel`:
  ``Lowering(Unsupported("composed Unit scalar call requires structural call
  custody"))`` at `src/unit/attached_unit/composed_control/admission.rs` for
  the `stamp(&mut output, ...)` out-parameter — structural call-custody
  territory (`validation/machine_calls/structural_call_custody.rs` and
  `src/unit/attached_unit*` are under the WRITE-ONLY-BORROW
  integer-entry-ranges claim). `source_replay_rejects_return_parameter_and_carrier_substitution`
  unwraps `plan.structural_result` at `tests/owned_record_return_source.rs:392`
  on `None` — the negative control finds no structural-result plan to
  tamper. `source_replay_requires_the_exact_affine_return_transfer`:
  `Record: changed return transfer 0` — mutating the return transfer's
  `machine_symbol` is not rejected by
  `terminal_production::TerminalProductionRequest::produce_artifact`, a
  source-replay verification gap. This is the continuing "scalar-return
  pure source custody" group.
- `established by` call-result qualification (1 test).
  `registered_callback_lifetime::interpreted_register_unregister_round_trip_drives_the_ledger`
  fails at source check with ``cannot establish call-result qualification
  `Registration::Live`: the exact invocation, authorized route or consumed
  qualified claims, and result correspondence are not proved`` from
  `typed-trees-to-checked-trees/src/checks/content/call_results.rs` — the
  fixture's `domain Registration::Live established by Registrar::register`
  route is the ENTRY-CONTENT-ROOTS documented residual ("`established by`
  establishment routes ... stay red until the receiver lifecycle leg
  lands"). `src/checks/content` is under the BOUNDARY-ISSUANCE claim.
- Proof-search blowup (1 test, SIGTERM).
  `nominal_affine_source::integer_comparison::mixed_nominal_integer_comparison_converges_before_one_shared_cleanup_return`
  was killed after ~1300s at ~573% CPU inside `lower_machine` on this host;
  the fixture (an 18-formal machine carrying ~60 requires conjuncts) is
  unchanged since August, so the cost is proof search, plausibly
  interacting with the bound-closure/premise relaxations of 2026-09-19/20
  (20ceb1e0a4, e37e1a8afc, a9f1a8aa8b, ebef2636d8) — not bisected, and
  kernel hardening on this host (`ptrace_scope=1`,
  `perf_event_paranoid=4`) blocked stack sampling. PROOF-SEARCH-MEASUREMENT
  owns `proof/src/checker.rs` and the per-plan measurement work
  (0750f6a18b); rerun alone via
  `cargo nextest run -p checked-trees-to-lowered-psi --test nominal_affine_source`
  to time it.

`cargo nextest run -p checked-trees-to-lowered-psi --no-fail-fast` at
9d0d864656 plus the anonymous-arithmetic repair beside this row (2026-09-18,
macOS arm64) runs the whole crate: 2032 run, 2012 passed, 20 failed. With the
`validation/affine_cleanup/continuation.rs` repair recorded in the
terminal-verifier section it is 1999 passed, 24 failed: that repair also
restores `unit_state_graph::bindings::structural_successors_reject_missing_and_surplus_arguments`,
which asserts the arity diagnostic from the producer side.

## compiler build-target activation

`mbx nextest run -p compiler --test build_target_activation` — 49/51 pass; 2
fail:
`x86_feature_admission::source_fma_then_attached_unit_call_stays_inside_one_canonical_mxcsr_envelope`
and
`x86_feature_admission::terminal_product_retains_exact_fma_operation_plan_and_x86_admission`,
both with `native artifact native instruction selection failed: FMA provider
transport is not implemented in the common instruction pipeline`. x86 FMA
provider transport is unimplemented; the failure is not host-specific.


## compiler `package_compilation_inputs`

`cargo nextest run -p compiler --test package_compilation_inputs
--no-fail-fast` at 63f625f942 plus the fixture repair beside this row
(2026-09-18, macOS arm64): 151 run, 149 passed, 2 failed, both Psi-side:

- `cross_package_visibility::public_dynamic_return_may_carry_private_producer_selected_evidence`:
  "state `code` requires an exact retained loan origin for its shared
  receiver". Not bisected.
- `module_constants::public_float_identity_requires_literals_with_matching_landings`:
  "computed constant leaf requires an exact builtin integer or Boolean
  carrier". Not bisected.

The three composition-mode admission failures earlier recorded on the
COMPONENT-SUBSTRATE board item are closed (0e6c25c4dc attributed, fixed at
65d71153a9).

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
--no-fail-fast` at 9cf696b9b5 plus the compile repair beside this row on
2026-09-17 (macOS arm64): 90 run, 82 passed, 8 failed, in three pre-existing
families. (65dc530cef removed `ArtifactEmissionPolicy` but left two
`with_artifact_policy` calls in this target, so it did not compile between
that commit and the repair; 51f21bb168 moved the emptied-scalar-contract
rejection to the earlier `scalar contract lost authored requirements` gate
and the expectation now names it.)

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
