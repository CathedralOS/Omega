//! Canonical direct-source fixture shared by MOVN corruption tests.

use crate::tests::*;

/// Test-only canonical construction. This helper grants no production
/// admission or policy authority.
pub(super) fn staged_realization() -> StagedPostAllocationMachineFunctionRelativeRealization {
    let integer_type = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let machine_fixture =
        conditional_immediate_machine(18_600, integer_type, [u128::from(u64::MAX), 7]);
    let module = conditional_immediate_module(machine_fixture.id, vec![machine_fixture]);
    let semantic = terminal_codec::encode_module(&module).unwrap();
    let proof = terminal_codec::encode_proof_bundle(&ProofBundle {
        recursive_components: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: Vec::new(),
    })
    .unwrap();
    let selections = OptimizationSelections::new([
        Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1,
    ])
    .unwrap();
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        ExplicitOptimizationRequest::new(selections, selected_lowering_budget()).unwrap(),
    )
    .unwrap();
    let target =
        lower_optimized_to_target_operations(optimized, NativeTarget::linux_arm64()).unwrap();
    let selected = stage_optimized_instruction_selection(target).unwrap();
    let liveness = stage_optimized_liveness(selected).unwrap();
    let ranges = stage_optimized_live_ranges(liveness).unwrap();
    let legality = stage_optimized_allocation_legality(ranges).unwrap();
    let homes = stage_optimized_register_homes(legality).unwrap();
    let machine = stage_optimized_post_allocation_machine_plan(&homes).unwrap();
    let optimization =
        stage_optimized_post_allocation_machine_optimization(&homes, &machine).unwrap();
    stage_post_allocation_machine_function_relative_realization(homes, machine, optimization)
        .unwrap()
}
