//! Canonical placement of disconnected functions without implicit padding.

use crate::tests::*;

#[test]
fn relocation_free_text_section_preserves_disconnected_function_order_without_padding() {
    let (semantic, proof) = conditional_exact_binary_artifact(false);
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
    let StagedOptimizedVerifiedPhysicalPipeline::FunctionRelativeLayout { realization } = physical
    else {
        panic!("rel8 must complete its direct function-relative realization")
    };
    let emitted = stage_optimized_function_fragment_emission(
        StagedOptimizedFunctionFragmentEmissionSource::X86Rel8Direct(Box::new(realization)),
    )
    .unwrap();
    let mut fragments = emitted.fragments().clone();
    let entry = fragments.entry;
    let first_length = fragments.functions[0].byte_count;
    let mut detached = fragments.functions[0].clone();
    detached.machine = MachineId::new(1).unwrap();
    fragments.functions.push(detached);
    fragments.identity = fragments.recomputed_identity();
    let expected_machines = [entry, MachineId::new(1).unwrap()];
    let mut placed =
        crate::stages::artifacts::function_fragment_text_section::place_fragments_for_test(
            &fragments,
        )
        .unwrap();
    assert_eq!(
        placed
            .functions
            .iter()
            .map(|function| function.machine)
            .collect::<Vec<_>>(),
        expected_machines
    );
    assert_eq!(placed.functions[0].section_offset, 0);
    assert_eq!(placed.functions[1].section_offset, first_length);
    assert_eq!(placed.semantic_entry, expected_machines[0]);
    assert_eq!(placed.semantic_entry_offset, 0);
    assert_eq!(placed.byte_count, first_length * 2);
    assert_eq!(
        placed.bytes,
        [
            fragments.functions[0].bytes.as_slice(),
            fragments.functions[1].bytes.as_slice(),
        ]
        .concat()
    );

    let replay =
        crate::stages::artifacts::function_fragment_text_section::place_fragments_for_test(
            &fragments,
        )
        .unwrap();
    assert_eq!(replay, placed);
    placed.functions.swap(0, 1);
    placed.identity = placed.recomputed_identity();
    assert_ne!(placed, replay);
}
