use crate::tests::*;
use omega_target::Architecture;

#[test]
fn exact_selection_is_deterministic_and_reaches_generic_publication_without_a_candidate() {
    let fixture = super::fixture(
        Optimization::Aarch64ElideSameViewCopyI64BeforeReturnV1,
        NativeTarget::linux_arm64(),
    );
    let first =
        stage_optimized_post_allocation_machine_optimization(&fixture.homes, &fixture.machine)
            .unwrap();
    let second =
        stage_optimized_post_allocation_machine_optimization(&fixture.homes, &fixture.machine)
            .unwrap();
    assert_eq!(first, second);
    assert_eq!(
        first.optimization(),
        Optimization::Aarch64ElideSameViewCopyI64BeforeReturnV1
    );
    assert_eq!(first.action_count(), 0);
    let StagedOptimizedPostAllocationMachineOptimization::Aarch64SameViewCopyElision(elision) =
        &first
    else {
        panic!("the exact selection must retain same-view-copy custody")
    };
    assert_eq!(elision.elision().plan().budget, budget());

    let realization = stage_post_allocation_machine_function_relative_realization(
        fixture.homes,
        fixture.machine,
        first,
    )
    .unwrap();
    assert_eq!(
        realization.optimization().optimization(),
        Optimization::Aarch64ElideSameViewCopyI64BeforeReturnV1
    );
    assert_eq!(realization.custody().optimization().action_count(), 0);
    assert_eq!(
        realization.custody().optimization().expected_byte_savings(),
        Some(0)
    );
    assert_eq!(
        realization.baseline_layout().functions(),
        realization.layout().functions()
    );
    validate_post_allocation_machine_function_relative_realization_custody(&realization).unwrap();
}

#[test]
fn compiler_generated_no_candidate_reaches_object_and_callable_publication() {
    let (semantic, proof) = conditional_exact_binary_artifact(false);
    let selections =
        OptimizationSelections::new([Optimization::Aarch64ElideSameViewCopyI64BeforeReturnV1])
            .unwrap();
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        ExplicitOptimizationRequest::new(selections.clone(), selected_lowering_budget()).unwrap(),
    )
    .unwrap();
    let physical = stage_optimized_verified_physical_pipeline_with_provider_executions(
        optimized,
        NativeTarget::linux_arm64(),
        &[],
    )
    .unwrap();
    let StagedOptimizedVerifiedPhysicalPipeline::PostAllocationMachine { realization } = physical
    else {
        panic!("the exact post-allocation selection must use the generic machine route")
    };
    assert_eq!(realization.optimization().action_count(), 0);

    let fragments = stage_optimized_function_fragment_emission(
        StagedOptimizedFunctionFragmentEmissionSource::PostAllocationMachine(Box::new(realization)),
    )
    .unwrap();
    let text = stage_optimized_relocation_free_text_section(fragments).unwrap();
    let object = stage_optimized_relocation_free_object_container(text).unwrap();
    let artifact =
        stage_validated_optimized_object_artifact(canonical_artifact(&semantic, &proof), object)
            .unwrap();
    assert_eq!(artifact.artifact().selections, selections.identity());
    let callable = stage_validated_optimized_ordinary_callable_entry(artifact).unwrap();
    assert_eq!(callable.entry().selections, selections.identity());
    validate_optimized_ordinary_callable_entry(&callable).unwrap();
}

#[test]
fn absent_exact_selection_and_wrong_architecture_fail_before_rule_execution() {
    let disabled = super::fixture(Optimization::CopyPropagation, NativeTarget::linux_arm64());
    assert_eq!(
        stage_optimized_aarch64_same_view_copy_elision(&disabled.homes, &disabled.machine),
        Err(
            OptimizedPostAllocationMachineOptimizationError::MissingPostAllocationMachineOptimization
        )
    );

    let wrong_target = super::fixture(
        Optimization::Aarch64ElideSameViewCopyI64BeforeReturnV1,
        NativeTarget::linux_x64(),
    );
    assert_eq!(
        stage_optimized_post_allocation_machine_optimization(
            &wrong_target.homes,
            &wrong_target.machine,
        ),
        Err(
            OptimizedPostAllocationMachineOptimizationError::UnsupportedPostAllocationMachineOptimizationTarget {
                optimization: Optimization::Aarch64ElideSameViewCopyI64BeforeReturnV1,
                required: Architecture::Aarch64,
                actual: Architecture::X86_64,
            }
        )
    );
}
