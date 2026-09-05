use crate::tests::*;

#[test]
fn frameless_exit_contract_rejects_unpreserved_x86_callee_saved_write() {
    let target = NativeTarget::linux_x64();
    let selections = OptimizationSelections::new([
        Optimization::CopyPropagation,
        Optimization::SelectedIncomingU12ExactAddImmediate,
    ])
    .unwrap();
    let legality = stage_optimized_allocation_legality(
        stage_optimized_live_ranges(
            stage_optimized_liveness(staged_exact_add_conditional_with_selections(
                target,
                selections,
                selected_lowering_budget(),
            ))
            .unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    let run = run_selected_lowering_optimizations(legality).unwrap();
    assert!(run.steps().is_empty());
    let homes = stage_optimized_register_homes_after_selected_lowering(run).unwrap();
    let rbx_units = homes
        .selected_lowering_run()
        .source_legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage()
        .register_environment()
        .physical()
        .model()
        .view_named("rbx")
        .unwrap()
        .units
        .clone();
    let error = stage_selected_lowering_function_relative_realization(homes.try_into().unwrap())
        .unwrap_err();
    let FunctionRelativeOptimizationRealizationError::ExitContract(
        WholeFunctionExitContractError::CalleeSavedWrite { instruction, unit },
    ) = error
    else {
        panic!("unpreserved RBX write must fail at the whole-function exit contract")
    };
    assert_eq!(instruction, selected_instructions::SelectedInstructionId(3));
    assert!(rbx_units.contains(&unit));
}
