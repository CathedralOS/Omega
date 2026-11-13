# Ledger bisect part 7 (k=7, N=40)

Measured commit (PIN): `51e8c45c793007f211d0d9baee7de16767172444`

## Summary

- Legacy report clusters in slice: **6** (report indices 7, 47, 87, 127, 167, 207)
- Fixture rows re-measured: **31**
- Fixed on PIN: **27**
- Still broken on PIN: **4**
- PIN diagnostic clusters (after regroup): **3**

## Corrected bisect (exponential first-parent walk)

Good commits are the first power-of-two first-parent distance from the PIN whose witness exits 0.
Exit 1 is only the PIN status together with the PIN diagnostic text. A fresh `CARGO_TARGET_DIR=/tmp/bisect-target-<sha>` is used for every commit.
The binary-search culprits, including any ledger-only commit, are withdrawn.

| cluster | good | first bad (PIN diagnostic) | stops matching | PR | triple at bad | triple at parent |
| --- | --- | --- | --- | ---: | --- | --- |
| countdown | `68b1302cf9ebc2a9297840db1deb3b6993b6ac3d` | `b9f7b5f007015207843632512dbf4311bf36b3d9` | `b9f7b5f007015207843632512dbf4311bf36b3d9` | 1113 | `[1, 1, 1]` | `[0, 0, 0]` |

Witness `pass/termination/measure_field_computed_limit` status `bisected`.
First-bad parent `ca20d0c585423571631b838f511d71ab4040416b`.
- `pass/termination/measure_computed_endpoint` at good `0`, at first bad `1`: terminating machine walk: cannot prove rank range `0..countdown.limit + 1` for the rank produced by the selected view
- `pass/termination/measure_field_computed_limit` at good `0`, at first bad `1`: terminating machine walk: cannot prove rank range `0..countdown.limit + 1` for the rank produced by the selected view

Hunk `omega-rust/psi/compilation/checking/src/lib.rs`:
```diff
     AssignmentStorageOrigins, AssignmentWriteTarget, CallFrameResolver, CallWriteOrigins,
     LocalWriteOrigin, LocalWriteSource, RecastSource, ReferenceResultLeafOrigin,
@@ -482,43 +482,44 @@ pub use proof_contracts::contract_entailment::collection_bounds::CollectionBound
 pub use proof_contracts::contract_entailment::is_arm_pattern_marker;
 pub use proof_contracts::contract_entailment::law_conformance::{
     InheritedRequirementApplication, inherited_requirement_proposition_application,
     inherited_requirement_proposition_label, inherited_satisfier_parameters,
 };
 pub use proof_contracts::contract_entailment::law_conformance::{
     MatchedLawGuarantee, matched_machine_law_guarantees,
 };
 pub use proof_contracts::contract_entailment::proof_integer::{
     anonymous_integer_math_literal, proof_integer_expression,
 };
 pub use proof_contracts::contract_entailment::ranking_range::{
-    ArithmeticCallObservation, CapturedRankEntry, RankArrivalCompatibility,
-    RankArrivalCorrespondence, RankArrivalDemand, RankCoordinateBinding, RankCoordinateTransport,
-    RankLocalCapture, RankPremiseCoordinate, RankPremiseInput, RankPremiseObservation,
-    RankProtectedPath, RankReceivingCoordinate, RankStateBindings, RankSubjectCoordinate,
-    RankValueDerivation, RankingRangeEdgeProof, RankingRangeEntryProof, RankingRangeMeasure,
-    RankingRangePremises, RankingRangeState, StateEntryCorrespondence,
+    ArithmeticCallObservation, CapturedRankApplicationInput, CapturedRankEntry,
+    RankArrivalCompatibility, RankArrivalCorrespondence, RankArrivalDemand, RankCoordinateBinding,
+    RankCoordinateTransport, RankLocalCapture, RankPremiseCoordinate, RankPremiseInput,
+    RankPremiseObservation, RankProtectedPath, RankReceivingCoordinate, RankStateBindings,
+    RankSubjectCoordinate, RankValueDerivation, RankingRangeEdgeProof, RankingRangeEntryProof,
+    RankingRangeMeasure, RankingRangePremises, RankingRangeState, StateEntryCorrespondence,
     arithmetic_entry_requirement_is_covered, discover_rank_arrival_correspondence,
     frame_preserves_rank_premises, protect_rank_premise, prove_arithmetic_call_requirement,
     prove_captured_arithmetic_call_requirement, prove_captured_ranking_range_entry,
     prove_captured_ranking_range_entry_with_formation, prove_ranking_range_edge,
     prove_ranking_range_entry, prove_ranking_range_entry_with_formation,
     prove_ranking_range_transition, rank_protected_path, ranking_coordinate_type,
     ranking_range_arrival_preserves_roles, ranking_range_premise_symbols,
     ranking_range_required_coordinates, ranking_range_state_premise_coordinates,
     ranking_slice_length_observation, select_rank_arrival_correspondence,
 };
 pub use proof_contracts::contract_entailment::ranking_range::{
-    ComputationBodyShape, DeclaredScalarView, MeasureBodyShape, ProjectionStep,
-    ScalarViewComputation, computation_body_shape, declared_scalar_view, find_declared_measure,
+    ComputationBodyShape, DeclaredInputRead, DeclaredInputShape, DeclaredScalarView,
+    MeasureBodyShape, ProjectionStep, ScalarViewComputation, computation_body_shape,
+    declared_input_shape, declared_input_type_matches, declared_scalar_view, find_declared_measure,
     identity_subject_matches, measure_body_shape, measure_constraints_cover_subject,
 };
 pub use proof_contracts::contract_entailment::refuted_requires::instantiated_value_call_requires_proven;
 pub use proof_contracts::contract_entailment::scoped_arithmetic::{
```

| family | pending | pending | pending |  |  |  |
| kernel | pending | pending | pending |  |  |  |
## PIN clusters (measurement only; earlier breaking commits withdrawn)

| rows | kind | diagnostic | witness | breaking commit | PR | mechanism | status |
| ---: | --- | --- | --- | --- | ---: | --- | --- |
| 2 | pass/run rejected | terminating machine walk: cannot prove rank range `0..countdown.limit + 1` for the rank produced by the selected view | `pass/termination/measure_field_computed_limit` | withdrawn |  | withdrawn | withdrawn |
Members:
- `pass/termination/measure_computed_endpoint` — still broken: terminating machine walk: cannot prove rank range `0..countdown.limit + 1` for the rank produced by the selected view
- `pass/termination/measure_field_computed_limit` — still broken: terminating machine walk: cannot prove rank range `0..countdown.limit + 1` for the rank produced by the selected view

| 1 | pass/run rejected | proposition `chained` expects 2 proof-static binder argument(s), received 0 | `pass/proofs/family_carrier_relation_laws` | withdrawn |  | withdrawn | withdrawn |
Members:
- `pass/proofs/family_carrier_relation_laws` — still broken: proposition `chained` expects 2 proof-static binder argument(s), received 0

| 1 | pass/run rejected | terminating machine walk: cannot prove rank range `0..limits.limit % limits.divisor + 6` for the rank produced by the selected view | `pass/proofs/kernel_integer_subtract_order` | withdrawn |  | withdrawn | withdrawn |
Members:
- `pass/proofs/kernel_integer_subtract_order` — still broken: terminating machine walk: cannot prove rank range `0..limits.limit % limits.divisor + 6` for the rank produced by the se

## Fixed on main (PIN)

- `fail/arithmetic/wrapping_target_plain_operands_rejected`
- `fail/facts/source_indexed_type_parameter_through_call_result_rejected`
- `fail/facts/source_indexed_type_parameter_through_const_name_rejected`
- `fail/facts/source_indexed_type_parameter_through_field_rejected`
- `fail/facts/source_indexed_type_parameter_through_local_rejected`
- `fail/facts/source_indexed_type_parameter_through_match_join_rejected`
- `fail/facts/source_indexed_type_parameter_through_other_machine_rejected`
- `fail/facts/source_indexed_type_parameter_through_other_state_rejected`
- `fail/facts/source_indexed_type_parameter_through_parameter_rejected`
- `fail/facts/source_indexed_type_parameter_through_target_body_rejected`
- `fail/facts/source_indexed_type_parameter_wrong_type_fiber_rejected`
- `fail/layouts/target_layout_call_checks_all_widths`
- `fail/proofs/kernel_integer_subtract_order_false_twin`
- `pass/borrows/foreign_output_captured_reference_exit`
- `pass/facts/source_indexed_type_parameter_exit`
- `pass/facts/source_indexed_type_parameter_through_call_result_exit`
- `pass/facts/source_indexed_type_parameter_through_const_name_exit`
- `pass/facts/source_indexed_type_parameter_through_field_exit`
- `pass/facts/source_indexed_type_parameter_through_local_exit`
- `pass/facts/source_indexed_type_parameter_through_match_join_exit`
- `pass/facts/source_indexed_type_parameter_through_other_machine_exit`
- `pass/facts/source_indexed_type_parameter_through_other_state_exit`
- `pass/facts/source_indexed_type_parameter_through_parameter_exit`
- `pass/facts/source_indexed_type_parameter_through_target_body_exit`
- `pass/facts/source_indexed_type_parameter_u64_payload_exit`
- `pass/layouts/runtime_recast_reference_captures_index_exit`
- `pass/recast/runtime_byte_region_state_transfer_exit`

## Legacy slice members (report cluster 2026-11-11)

### report index 7 (report rows=18)
- Witness: `fail/facts/source_indexed_type_parameter_through_call_result_rejected`
- Report diagnostic prefix: source constructor predicate derivation failed: Kernel(UnboundLevelParameter { index: 0, arity:
- `fail/facts/source_indexed_type_parameter_through_call_result_rejected` — fixed on main
- `fail/facts/source_indexed_type_parameter_through_const_name_rejected` — fixed on main
- `fail/facts/source_indexed_type_parameter_through_field_rejected` — fixed on main
- `fail/facts/source_indexed_type_parameter_through_local_rejected` — fixed on main
- `fail/facts/source_indexed_type_parameter_through_match_join_rejected` — fixed on main
- `fail/facts/source_indexed_type_parameter_through_other_machine_rejected` — fixed on main
- `fail/facts/source_indexed_type_parameter_through_other_state_rejected` — fixed on main
- `fail/facts/source_indexed_type_parameter_through_parameter_rejected` — fixed on main
- `fail/facts/source_indexed_type_parameter_through_target_body_rejected` — fixed on main
- `fail/facts/source_indexed_type_parameter_wrong_type_fiber_rejected` — fixed on main
- `pass/facts/source_indexed_type_parameter_exit` — fixed on main
- `pass/facts/source_indexed_type_parameter_through_call_result_exit` — fixed on main
- `pass/facts/source_indexed_type_parameter_through_const_name_exit` — fixed on main
- `pass/facts/source_indexed_type_parameter_through_field_exit` — fixed on main
- `pass/facts/source_indexed_type_parameter_through_local_exit` — fixed on main
- `pass/facts/source_indexed_type_parameter_through_match_join_exit` — fixed on main
- `pass/facts/source_indexed_type_parameter_through_other_machine_exit` — fixed on main
- `pass/facts/source_indexed_type_parameter_through_other_state_exit` — fixed on main
- `pass/facts/source_indexed_type_parameter_through_parameter_exit` — fixed on main
- `pass/facts/source_indexed_type_parameter_through_target_body_exit` — fixed on main
- `pass/facts/source_indexed_type_parameter_u64_payload_exit` — fixed on main

### report index 47 (report rows=4)
- Witness: `pass/proofs/kernel_integer_subtract_order`
- Report diagnostic prefix: terminating machine walk: cannot prove rank range `0..limits.limit % limits.divisor + 6` for the
- `pass/proofs/kernel_integer_subtract_order` — still broken
- `fail/proofs/kernel_integer_subtract_order_false_twin` — fixed on main
- `pass/termination/measure_computed_endpoint` — still broken
- `pass/termination/measure_field_computed_limit` — still broken

### report index 87 (report rows=2)
- Witness: `pass/layouts/runtime_recast_reference_captures_index_exit`
- Report diagnostic prefix: duplicate machine `_`
- `pass/layouts/runtime_recast_reference_captures_index_exit` — fixed on main
- `fail/layouts/target_layout_call_checks_all_widths` — fixed on main
- `pass/recast/runtime_byte_region_state_transfer_exit` — fixed on main

### report index 127 (report rows=1)
- Witness: `pass/proofs/family_carrier_relation_laws`
- Report diagnostic prefix: authored declaration typing could not extend its retained source: Lowering(Diagnostic { severity
- `pass/proofs/family_carrier_relation_laws` — still broken

### report index 167 (report rows=1)
- Witness: `pass/borrows/foreign_output_captured_reference_exit`
- Report diagnostic prefix: completed output has no captured lifetime-input receipt; completed output from `again` in machin
- `pass/borrows/foreign_output_captured_reference_exit` — fixed on main

### report index 207 (report rows=1)
- Witness: `fail/arithmetic/wrapping_target_plain_operands_rejected`
- Report diagnostic prefix: store in machine `Main::pick` state `pick` local `v` implicitly converts `i32` to `i32 in Wrappi
- `fail/arithmetic/wrapping_target_plain_operands_rejected` — fixed on main

## Provenance

- Host: linux x86_64
- Jobs: `OMEGA_CORPUS_JOBS=1`
- Commands:
  ```bash
  mbx build -p omega < /dev/null
  mbx test --no-run -p compiler --test corpus_runner < /dev/null
  OMEGA_CORPUS_FIXTURE_FILTER=<fixtures> OMEGA_CORPUS_RECORDS=.agents/ledger-bisect-2026-11-11/pin_records.txt \
    target/debug/build/compiler/*/out/corpus_runner-* < /dev/null
  ```
