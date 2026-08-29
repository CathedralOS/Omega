use omega_optimization_core::{Optimization, OptimizationExecutionPhase, OptimizationSelections};

use super::{
    AllocationRecoveryRuleCatalogError, ORDERED_ALLOCATION_RECOVERY_RULES,
    ORDERED_SELECTED_LOWERING_RULES, selected_allocation_recovery_rule,
    selected_lowering_rule_policy,
};

#[test]
fn catalog_exactly_matches_the_allocation_recovery_vocabulary() {
    let declared = Optimization::ALL
        .into_iter()
        .filter(|optimization| {
            optimization.execution_phase() == OptimizationExecutionPhase::AllocationRecovery
        })
        .collect::<Vec<_>>();
    assert_eq!(declared, ORDERED_ALLOCATION_RECOVERY_RULES);
    for optimization in ORDERED_ALLOCATION_RECOVERY_RULES {
        let selections = OptimizationSelections::new([optimization]).unwrap();
        assert_eq!(
            selected_allocation_recovery_rule(&selections),
            Ok(Some(optimization))
        );
    }
    assert_eq!(
        selected_allocation_recovery_rule(
            &OptimizationSelections::new(ORDERED_ALLOCATION_RECOVERY_RULES).unwrap()
        ),
        Err(AllocationRecoveryRuleCatalogError::UnsupportedComposition)
    );
}

#[test]
fn catalog_exactly_matches_the_selected_lowering_vocabulary() {
    let declared = Optimization::ALL
        .into_iter()
        .filter(|optimization| {
            optimization.execution_phase() == OptimizationExecutionPhase::SelectedLowering
        })
        .collect::<Vec<_>>();
    assert_eq!(declared, ORDERED_SELECTED_LOWERING_RULES);
    for optimization in ORDERED_SELECTED_LOWERING_RULES {
        let selections = OptimizationSelections::new([optimization]).unwrap();
        assert!(selected_lowering_rule_policy(&selections).is_ok());
    }
    let composition = OptimizationSelections::new(ORDERED_SELECTED_LOWERING_RULES).unwrap();
    assert!(selected_lowering_rule_policy(&composition).is_ok());
}
