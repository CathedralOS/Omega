//! Exact selected-lowering compositions admitted through fragment and artifact custody.

use crate::tests::*;

#[test]
fn relocation_free_fragment_emission_accepts_both_selected_lowering_compositions() {
    let (semantic, proof) = conditional_exact_binary_artifact(false);
    let x86_selections = OptimizationSelections::new([
        Optimization::SelectedIncomingU12ExactAddImmediate,
        Optimization::X86RelaxConditionalBranchesToRel8V1,
    ])
    .unwrap();
    let x86_optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        ExplicitOptimizationRequest::new(x86_selections, selected_lowering_budget()).unwrap(),
    )
    .unwrap();
    let x86_physical = stage_optimized_verified_physical_pipeline_with_provider_executions(
        x86_optimized,
        NativeTarget::linux_x64(),
        &[],
    )
    .unwrap();
    let realization = (x86_physical)
        .into_selected_lowering_for_test()
        .unwrap_or_else(|| panic!("combined x86 suite must retain selected-lowering custody"));
    let x86 = stage_optimized_function_fragment_emission(
        StagedOptimizedFunctionFragmentEmissionSource::SelectedLowering(Box::new(realization)),
    )
    .unwrap();
    assert_eq!(
        validate_optimized_function_fragment_emission(&x86).unwrap(),
        x86.custody()
    );
    let x86 = stage_optimized_relocation_free_text_section(x86).unwrap();
    assert_eq!(
        validate_optimized_relocation_free_text_section(&x86).unwrap(),
        x86.custody()
    );
    let x86 = stage_optimized_relocation_free_object_container(x86).unwrap();
    assert_eq!(
        validate_optimized_relocation_free_object_container(&x86).unwrap(),
        x86.custody()
    );
    let x86 = stage_validated_optimized_object_artifact(canonical_artifact(&semantic, &proof), x86)
        .unwrap();
    assert_eq!(
        validate_optimized_object_artifact(&x86).unwrap(),
        x86.custody()
    );

    let arm_selections = OptimizationSelections::new([
        Optimization::SelectedIncomingU12ExactAddImmediate,
        Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1,
    ])
    .unwrap();
    let arm_optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        ExplicitOptimizationRequest::new(arm_selections, selected_lowering_budget()).unwrap(),
    )
    .unwrap();
    let arm_physical = stage_optimized_verified_physical_pipeline_with_provider_executions(
        arm_optimized,
        NativeTarget::linux_arm64(),
        &[],
    )
    .unwrap();
    let realization = (arm_physical)
        .into_post_allocation_machine_for_test()
        .unwrap_or_else(|| panic!("combined AArch64 suite must retain both phase completions"));
    let arm = stage_optimized_function_fragment_emission(
        StagedOptimizedFunctionFragmentEmissionSource::PostAllocationMachine(Box::new(realization)),
    )
    .unwrap();
    assert_eq!(
        validate_optimized_function_fragment_emission(&arm).unwrap(),
        arm.custody()
    );
    let arm = stage_optimized_relocation_free_text_section(arm).unwrap();
    assert_eq!(
        validate_optimized_relocation_free_text_section(&arm).unwrap(),
        arm.custody()
    );
    let arm = stage_optimized_relocation_free_object_container(arm).unwrap();
    assert_eq!(
        validate_optimized_relocation_free_object_container(&arm).unwrap(),
        arm.custody()
    );
    let arm = stage_validated_optimized_object_artifact(canonical_artifact(&semantic, &proof), arm)
        .unwrap();
    assert_eq!(
        validate_optimized_object_artifact(&arm).unwrap(),
        arm.custody()
    );
}
