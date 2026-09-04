//! Aggregate selected-lowering budget enforcement across component attempts.

use crate::tests::*;

#[test]
fn selected_lowering_suite_enforces_one_aggregate_budget() {
    let target = NativeTarget::linux_x64();
    let selections = OptimizationSelections::new([
        Optimization::CopyPropagation,
        Optimization::SelectedIncomingU12ExactAddImmediate,
    ])
    .unwrap();
    let source = stage_optimized_allocation_legality(
        stage_optimized_live_ranges(
            stage_optimized_liveness(staged_exact_add_conditional_with_selections(
                target,
                selections.clone(),
                selected_lowering_budget(),
            ))
            .unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    let reference = run_selected_lowering_optimizations(source).unwrap();
    let attempt = reference.attempt();
    let component_usages = [
        attempt.choices().receipt().usage(),
        attempt.recovery().receipt().usage(),
        attempt.fold().receipt().usage(),
    ];
    let maximum = |field: fn(OptimizationWorkUsage) -> u64| {
        component_usages
            .into_iter()
            .map(field)
            .max()
            .unwrap()
            .max(1)
    };
    let component_only_budget = OptimizationWorkBudget::new(
        maximum(|usage| usage.rule_evaluations),
        maximum(|usage| usage.candidates),
        maximum(|usage| usage.validation_steps),
        maximum(|usage| usage.commits),
        maximum(|usage| usage.iterations),
    )
    .unwrap();
    assert!(
        component_usages
            .into_iter()
            .all(|usage| usage.within(component_only_budget))
    );

    let source = stage_optimized_allocation_legality(
        stage_optimized_live_ranges(
            stage_optimized_liveness(staged_exact_add_conditional_with_selections(
                target,
                selections,
                component_only_budget,
            ))
            .unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    assert!(matches!(
        run_selected_lowering_optimizations(source),
        Err(OptimizedLiteralFoldCustodyError::SelectedLoweringBudgetExceeded { .. })
    ));
}
