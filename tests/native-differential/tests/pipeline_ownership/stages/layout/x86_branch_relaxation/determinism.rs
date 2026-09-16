//! Repeated staging and phase execution over identical inputs return identical artifacts.
use crate::tests::{
    Optimization, OptimizationSelections, execute_resolved_layout_optimization,
    selected_lowering_budget, stage_optimized_layout_independent_selected_form_encoding,
    stage_optimized_post_allocation_machine_plan, stage_optimized_resolved_selected_form_layout,
    stage_optimized_x86_branch_relaxation,
};
use optimization_core::OptimizationExecutionPhase;

#[test]
fn relaxation_and_phase_artifacts_are_deterministic_across_repeated_runs() {
    // Two fully independent stagings of the same source produce identical
    // evidence: identities, attempts, actions, usage, and retained layout.
    let first = super::fixture::stage_with_budget(selected_lowering_budget()).unwrap();
    let second = super::fixture::stage_with_budget(selected_lowering_budget()).unwrap();
    assert_eq!(first, second);

    // Re-running the rule and the phase entrance on one staged input set is
    // deterministic for both the disabled identity leg and the enabled rule.
    let homes = super::fixture::physical_homes();
    let machine = stage_optimized_post_allocation_machine_plan(&homes).unwrap();
    let selected_stage = homes
        .legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    let selected = selected_stage.selected();
    let physical = selected_stage.register_environment().physical();
    let encoding = stage_optimized_layout_independent_selected_form_encoding(
        selected, &machine, physical, None,
    )
    .unwrap();
    let baseline =
        stage_optimized_resolved_selected_form_layout(selected, &machine, physical, &encoding)
            .unwrap();
    assert_eq!(
        stage_optimized_x86_branch_relaxation(
            selected,
            &machine,
            physical,
            &encoding,
            &baseline,
            selected_lowering_budget(),
        ),
        stage_optimized_x86_branch_relaxation(
            selected,
            &machine,
            physical,
            &encoding,
            &baseline,
            selected_lowering_budget(),
        ),
    );
    for selections in [
        OptimizationSelections::new([])
            .unwrap()
            .project_phase(OptimizationExecutionPhase::FunctionRelativeLayout),
        OptimizationSelections::new([Optimization::X86RelaxConditionalBranchesToRel8V1])
            .unwrap()
            .project_phase(OptimizationExecutionPhase::FunctionRelativeLayout),
    ] {
        assert_eq!(
            execute_resolved_layout_optimization(
                selected,
                &machine,
                physical,
                &encoding,
                &baseline,
                &selections,
                selected_lowering_budget(),
            ),
            execute_resolved_layout_optimization(
                selected,
                &machine,
                physical,
                &encoding,
                &baseline,
                &selections,
                selected_lowering_budget(),
            ),
        );
    }
}
