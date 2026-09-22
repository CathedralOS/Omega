use crate::FunctionFragmentReplayInputs;
use crate::tests::{
    AdmissionProfile, AllocationReplayError, ExplicitOptimizationRequest,
    FunctionFragmentFrameApplicationIdentity, FunctionRelativeOptimizationRealizationError,
    NativeTarget, Optimization, OptimizationSelections, OptimizationWorkBudget,
    OptimizedTargetLoweringRequest, StagedValidatedOptimizedOrdinaryCallableEntry,
    canonical_artifact, conditional_u64_not_equal_zero_parameter_artifact,
    lower_optimized_to_target_operations,
    optimization_pipeline_report_from_ordinary_callable_entry, optimize_artifact_sections,
    selected_lowering_budget, stage_fixed_frame_function_relative_realization,
    stage_function_fragment_frame_application, stage_optimized_allocation_legality,
    stage_optimized_fixed_frame_text_section, stage_optimized_function_fragment_emission,
    stage_optimized_instruction_selection, stage_optimized_live_ranges, stage_optimized_liveness,
    stage_optimized_register_homes, stage_optimized_relocation_free_object_container,
    stage_validated_optimized_object_artifact, stage_validated_optimized_ordinary_callable_entry,
    staged_exact_add_conditional, validate_fixed_frame_function_relative_realization,
    validate_optimized_ordinary_callable_entry,
};
use selected_instructions_to_register_homes::AllocationSource;
use selected_instructions_to_register_homes::ValidatedSelectedAnalysis;

#[test]
fn fixed_frame_rejects_a_machine_from_another_allocation_before_encoding() {
    let allocate = |target| {
        let selected = staged_exact_add_conditional(target);
        selected_instructions_to_register_homes::stage_register_allocation(
            selected_instructions_to_register_homes::optimize_selected_instructions(selected)
                .unwrap(),
        )
        .unwrap()
    };
    let foreign = |target: NativeTarget| match target.architecture {
        target::Architecture::X86_64 => NativeTarget::linux_arm64(),
        target::Architecture::Aarch64 => NativeTarget::linux_x64(),
    };
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let allocation = allocate(target);
        let other = allocate(foreign(target));
        let machine =
            register_homes_to_post_allocation_machine::stage_optimized_post_allocation_machine_plan(
                &other.current(),
            )
            .unwrap();
        assert!(matches!(
            stage_fixed_frame_function_relative_realization(
                allocation,
                machine,
                selected_lowering_budget()
            ),
            Err(FunctionRelativeOptimizationRealizationError::PostAllocationMachine(_))
        ));
    }
}

#[test]
fn fixed_frame_retains_original_allocation_and_rejects_current_program_substitution() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        for replace_selected in [false, true] {
            let selected = staged_exact_add_conditional(target);
            let ranges =
                stage_optimized_live_ranges(stage_optimized_liveness(selected).unwrap()).unwrap();
            let homes = stage_optimized_register_homes(
                stage_optimized_allocation_legality(ranges).unwrap(),
            )
            .unwrap();
            let current = homes.replay_allocation().unwrap();
            let selected_owner = current.selected().shared_selected_plan();
            let home_owner = current.homes().shared_plan();
            let mut realization = crate::tests::with_allocated_machine(
                homes.try_into().unwrap(),
                |allocation, machine| {
                    stage_fixed_frame_function_relative_realization(
                        allocation,
                        machine,
                        selected_lowering_budget(),
                    )
                },
            )
            .unwrap();
            assert!(std::sync::Arc::ptr_eq(
                &selected_owner,
                &realization.allocation().program().selected
            ));
            assert!(std::sync::Arc::ptr_eq(
                &home_owner,
                &realization.allocation().program().homes
            ));
            validate_fixed_frame_function_relative_realization(&realization).unwrap();
            let mut substituted = realization.allocation().program().clone();
            if replace_selected {
                std::sync::Arc::make_mut(&mut substituted.selected)
                    .functions
                    .clear();
            } else {
                std::sync::Arc::make_mut(&mut substituted.homes)
                    .functions
                    .clear();
            }
            realization
                .allocation_mut()
                .substitute_current_program_for_test(substituted);
            assert!(matches!(
                validate_fixed_frame_function_relative_realization(&realization),
                Err(FunctionRelativeOptimizationRealizationError::Allocation(
                    AllocationReplayError::CurrentProgramMismatch
                ))
            ));
            assert!(
                stage_optimized_function_fragment_emission(
                    FunctionFragmentReplayInputs::from(realization).into(),
                )
                .is_err()
            );
        }
    }
}

fn staged_fixed_frame_callable(
    target: NativeTarget,
) -> (
    StagedValidatedOptimizedOrdinaryCallableEntry,
    FunctionFragmentFrameApplicationIdentity,
) {
    let (semantic, proof) = conditional_u64_not_equal_zero_parameter_artifact();
    let selections = OptimizationSelections::new([Optimization::CopyPropagation]).unwrap();
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        ExplicitOptimizationRequest::new(selections, selected_lowering_budget()).unwrap(),
    )
    .unwrap();
    let target = lower_optimized_to_target_operations(
        optimized,
        OptimizedTargetLoweringRequest::new(target),
    )
    .unwrap();
    let selected = stage_optimized_instruction_selection(target).unwrap();
    let liveness = stage_optimized_liveness(selected).unwrap();
    let ranges = stage_optimized_live_ranges(liveness).unwrap();
    let legality = stage_optimized_allocation_legality(ranges).unwrap();
    let homes = stage_optimized_register_homes(legality).unwrap();
    let budget =
        OptimizationWorkBudget::new(1_000_000, 1_000_000, 1_000_000, 1_000_000, 1_000_000).unwrap();
    let realization =
        crate::tests::with_allocated_machine(homes.try_into().unwrap(), |allocation, machine| {
            stage_fixed_frame_function_relative_realization(allocation, machine, budget)
        })
        .unwrap();
    let fragments = stage_optimized_function_fragment_emission(
        FunctionFragmentReplayInputs::from(realization).into(),
    )
    .unwrap();
    let applied = stage_function_fragment_frame_application(fragments).unwrap();
    let application = applied.receipt().identity();
    let text = stage_optimized_fixed_frame_text_section(applied).unwrap();
    let object = stage_optimized_relocation_free_object_container(text).unwrap();
    let artifact =
        stage_validated_optimized_object_artifact(canonical_artifact(&semantic, &proof), object)
            .unwrap();
    (
        stage_validated_optimized_ordinary_callable_entry(artifact).unwrap(),
        application,
    )
}

#[test]
fn fixed_frame_source_reaches_ordinary_callable_on_both_isas() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let (callable, application) = staged_fixed_frame_callable(target);
        assert_eq!(
            validate_optimized_ordinary_callable_entry(&callable).unwrap(),
            callable.custody()
        );
        let fixed = callable.source().source().source();
        assert_eq!(fixed.manifest().record().frame_application, application);
        let report = optimization_pipeline_report_from_ordinary_callable_entry(&callable);
        assert_eq!(
            report.ordinary_callable_entry().unwrap().entry,
            callable.entry().identity
        );
    }
}
