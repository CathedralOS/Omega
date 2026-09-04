use crate::tests::*;
use omega_optimization_core::OptimizationExecutionPhase;
use omega_regalloc::{
    LiteralFoldError, SelectedLoweringRuleCatalogError, resolve_selected_lowering_rules,
};

use super::fixture::*;

#[test]
fn exact_subtract_rule_is_disabled_by_the_default_selected_lowering_projection() {
    let disabled = OptimizationSelections::default();
    assert_eq!(
        resolve_selected_lowering_rules(&disabled),
        Err(SelectedLoweringRuleCatalogError::MissingSelection)
    );

    for (target, sole_view_name) in targets() {
        // Empty global explicit selections are intentionally unrepresentable.
        // An unrelated Psi selection carries the pressure shape while retaining
        // the canonical empty selected-lowering projection.
        let source = source_with_selections(
            target,
            sole_view_name,
            OptimizationSelections::new([Optimization::CopyPropagation]).unwrap(),
            selected_lowering_budget(),
        );
        let selected_stage = source.live_range_stage().liveness_stage().selected_stage();
        assert_eq!(
            selected_stage
                .optimized_target()
                .optimized()
                .selections()
                .for_phase(OptimizationExecutionPhase::SelectedLowering),
            disabled
        );

        let environment = selected_stage.register_environment();
        let choices = choose_spill_victims(
            source.legality(),
            source.live_range_stage().ranges(),
            environment.identity(),
            environment.physical(),
            environment.constraints(),
            environment.reservations(),
            environment.allocation_constraint_keys(),
            SpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
            selected_lowering_budget(),
        )
        .unwrap();
        assert!(
            choices
                .plan()
                .functions
                .iter()
                .any(|function| function.choice.is_some())
        );
        assert!(matches!(
            run_selected_lowering_optimizations(source),
            Err(OptimizedLiteralFoldCustodyError::MissingSelectedLoweringOptimization)
        ));
    }
}

#[test]
fn exact_subtract_rule_does_not_fall_back_from_an_exact_add_selection() {
    for (target, sole_view_name) in targets() {
        let first = run(target, sole_view_name, false, selected_lowering_budget()).unwrap_err();
        let repeated = run(target, sole_view_name, false, selected_lowering_budget()).unwrap_err();
        assert_eq!(first, repeated);
        assert_eq!(
            first,
            OptimizedLiteralFoldCustodyError::Fold(LiteralFoldError::ConsumerMismatch {
                function: 0,
            })
        );
    }
}

#[test]
fn exact_subtract_rule_is_target_independent_deterministic_and_reaches_a_validated_fixed_point() {
    for (target, sole_view_name) in targets() {
        let budget = selected_lowering_budget();
        let first = run(target, sole_view_name, true, budget).unwrap();
        let repeated = run(target, sole_view_name, true, budget).unwrap();

        assert_eq!(first.custody(), repeated.custody());
        assert_eq!(first.steps(), repeated.steps());
        assert_eq!(first.attempt(), repeated.attempt());
        assert_eq!(first.steps().len(), 2);
        assert_eq!(first.custody().action_count(), 2);
        // This is the runner's independently validated terminal attempt, not a
        // claim that its transformed selected plan is a fresh public-stage input.
        assert_eq!(first.attempt().fold().receipt().applied_count(), 0);
        assert_eq!(
            first.attempt().fold().receipt().source_selected(),
            first.attempt().fold().receipt().transformed_selected()
        );
        assert_eq!(
            validate_selected_lowering_optimization_custody(&first).unwrap(),
            *first.custody()
        );

        let immediates = first.attempt().fold().transformed().functions[0]
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter_map(|instruction| match instruction.kind {
                SelectedInstructionKind::ExactSubtractI64Immediate { immediate, .. } => {
                    Some(immediate)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            immediates,
            [IntegerValue::Unsigned(5), IntegerValue::Unsigned(8)]
        );
    }
}
