use crate::tests::*;
use selected_instructions_to_register_homes::RetainedAllocation;

pub(super) fn allocation(
    target: NativeTarget,
    lowering: bool,
    relaxation: bool,
) -> RetainedAllocation {
    let mut selections = vec![Optimization::CopyPropagation];
    if lowering {
        selections.push(Optimization::SelectedIncomingU12ExactAddImmediate);
    }
    if relaxation {
        selections.push(Optimization::X86RelaxConditionalBranchesToRel8V1);
    }
    let selected = staged_exact_add_conditional_with_selections(
        target,
        OptimizationSelections::new(selections).unwrap(),
        selected_lowering_budget(),
    );
    let ranges = stage_optimized_live_ranges(stage_optimized_liveness(selected).unwrap()).unwrap();
    let legality = stage_optimized_allocation_legality_for_frameless_leaf(ranges).unwrap();
    if lowering {
        let run = run_selected_lowering_optimizations(legality).unwrap();
        stage_optimized_register_homes_after_selected_lowering(run)
            .unwrap()
            .try_into()
            .unwrap()
    } else {
        stage_optimized_register_homes(legality)
            .unwrap()
            .try_into()
            .unwrap()
    }
}

fn change_current_program(allocation: &mut RetainedAllocation, selected: bool) {
    let mut changed = allocation.program().clone();
    if selected {
        std::sync::Arc::make_mut(&mut changed.selected)
            .functions
            .clear();
    } else {
        std::sync::Arc::make_mut(&mut changed.homes)
            .functions
            .clear();
    }
    allocation.substitute_current_program_for_test(changed);
}

#[test]
fn selected_lowering_realization_owns_current_data_independently_of_replay() {
    for (target, relaxation) in [
        (NativeTarget::linux_x64(), false),
        (NativeTarget::linux_x64(), true),
        (NativeTarget::linux_arm64(), false),
    ] {
        let allocation = allocation(target, true, relaxation);
        let original = allocation.program().clone();
        let mut realization =
            stage_selected_lowering_function_relative_realization(allocation).unwrap();
        assert!(std::sync::Arc::ptr_eq(
            &original.selected,
            &realization.allocation().program().selected
        ));
        assert!(std::sync::Arc::ptr_eq(
            &original.homes,
            &realization.allocation().program().homes
        ));
        assert_eq!(realization.relaxation().is_some(), relaxation);
        let custody =
            validate_selected_lowering_function_relative_realization_custody(&realization).unwrap();
        for selected in [false, true] {
            change_current_program(realization.allocation_mut(), selected);
            assert_eq!(
                validate_selected_lowering_function_relative_realization_custody(&realization),
                Err(FunctionRelativeOptimizationRealizationError::Allocation(
                    AllocationReplayError::CurrentProgramMismatch
                ))
            );
            realization
                .allocation_mut()
                .substitute_current_program_for_test(original.clone());
            assert_eq!(
                validate_selected_lowering_function_relative_realization_custody(&realization)
                    .unwrap(),
                custody
            );
        }
        drop(realization);
        assert!(!original.selected.functions.is_empty());
        assert!(!original.homes.functions.is_empty());
    }
}

#[test]
fn branch_relaxation_realization_owns_current_data_independently_of_replay() {
    let allocation = allocation(NativeTarget::linux_x64(), false, true);
    let original = allocation.program().clone();
    let mut realization =
        stage_function_relative_layout_optimization_realization(allocation).unwrap();
    assert!(std::sync::Arc::ptr_eq(
        &original.selected,
        &realization.allocation().program().selected
    ));
    assert!(std::sync::Arc::ptr_eq(
        &original.homes,
        &realization.allocation().program().homes
    ));
    let custody =
        validate_function_relative_layout_optimization_realization_custody(&realization).unwrap();
    for selected in [false, true] {
        change_current_program(realization.allocation_mut(), selected);
        assert_eq!(
            validate_function_relative_layout_optimization_realization_custody(&realization),
            Err(FunctionRelativeOptimizationRealizationError::Allocation(
                AllocationReplayError::CurrentProgramMismatch
            ))
        );
        realization
            .allocation_mut()
            .substitute_current_program_for_test(original.clone());
        assert_eq!(
            validate_function_relative_layout_optimization_realization_custody(&realization)
                .unwrap(),
            custody
        );
    }
}

#[test]
fn exit_contract_data_outlives_its_producer_without_granting_admission() {
    for (target, relaxation) in [
        (NativeTarget::linux_x64(), false),
        (NativeTarget::linux_x64(), true),
        (NativeTarget::linux_arm64(), false),
    ] {
        let mut realization = stage_selected_lowering_function_relative_realization(allocation(
            target, true, relaxation,
        ))
        .unwrap();
        let original: std::sync::Arc<machine_code::WholeFunctionExitContract> =
            realization.exit_contract().shared_contract();
        assert!(std::sync::Arc::ptr_eq(
            &original,
            &realization.exit_contract().shared_contract(),
        ));
        assert_eq!(original.identity, original.recomputed_identity());
        let custody =
            validate_selected_lowering_function_relative_realization_custody(&realization).unwrap();

        let changed = realization.exit_contract_mut().contract_mut();
        changed.stack_alignment += 1;
        changed.identity = changed.recomputed_identity();
        assert_ne!(original.identity, changed.identity);
        assert_eq!(
            validate_selected_lowering_function_relative_realization_custody(&realization),
            Err(FunctionRelativeOptimizationRealizationError::ExitContract(
                WholeFunctionExitContractError::ArtifactMismatch,
            )),
        );
        *realization.exit_contract_mut().contract_mut() = (*original).clone();
        assert_eq!(
            validate_selected_lowering_function_relative_realization_custody(&realization).unwrap(),
            custody,
        );
        drop(realization);
        assert!(!original.functions.is_empty());
        assert_eq!(original.identity, original.recomputed_identity());
    }
}

#[test]
fn realization_receipt_roles_cannot_be_substituted_at_the_common_allocation_entrance() {
    assert!(matches!(
        stage_selected_lowering_function_relative_realization(allocation(
            NativeTarget::linux_x64(),
            false,
            true
        )),
        Err(FunctionRelativeOptimizationRealizationError::RootMismatch)
    ));
    assert!(matches!(
        stage_function_relative_layout_optimization_realization(allocation(
            NativeTarget::linux_x64(),
            true,
            true
        )),
        Err(FunctionRelativeOptimizationRealizationError::RootMismatch)
    ));
}
