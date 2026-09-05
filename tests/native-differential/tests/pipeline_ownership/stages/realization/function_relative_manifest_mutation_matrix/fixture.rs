//! Public-route function-relative fixtures shared by mutation leaves.

use crate::tests::*;

pub(super) fn direct_rel8_realization() -> StagedFunctionRelativeLayoutOptimizationRealization {
    direct_rel8_realization_for(false)
}

pub(super) fn alternate_direct_rel8_realization()
-> StagedFunctionRelativeLayoutOptimizationRealization {
    direct_rel8_realization_for(true)
}

fn direct_rel8_realization_for(
    subtract: bool,
) -> StagedFunctionRelativeLayoutOptimizationRealization {
    let (semantic, proof) = conditional_exact_binary_artifact(subtract);
    let selections =
        OptimizationSelections::new([Optimization::X86RelaxConditionalBranchesToRel8V1]).unwrap();
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        ExplicitOptimizationRequest::new(selections, selected_lowering_budget()).unwrap(),
    )
    .unwrap();
    let physical = stage_optimized_verified_physical_pipeline_with_provider_executions(
        optimized,
        NativeTarget::linux_x64(),
        &[],
    )
    .unwrap();
    (physical)
        .into_function_relative_layout_for_test()
        .unwrap_or_else(|| {
            panic!("the exact rel8 selection must use direct function-relative realization")
        })
}

pub(super) fn post_allocation_realization() -> StagedPostAllocationMachineFunctionRelativeRealization
{
    let integer_type = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let machine_fixture = conditional_immediate_machine(28_201, integer_type, [1, u32::MAX.into()]);
    let module = conditional_immediate_module(machine_fixture.id, vec![machine_fixture]);
    let semantic = terminal_codec::encode_module(&module).unwrap();
    let proof = terminal_codec::encode_proof_bundle(&ProofBundle {
        recursive_components: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: Vec::new(),
    })
    .unwrap();
    let selections = OptimizationSelections::new([
        Optimization::X86SelectMovR32Imm32ZeroExtendedI64MaterializationV1,
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
        lower_optimized_to_target_operations(optimized, NativeTarget::linux_x64()).unwrap();
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
