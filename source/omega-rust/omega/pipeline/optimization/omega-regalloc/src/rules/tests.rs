use omega_optimization_core::{Optimization, OptimizationExecutionPhase, OptimizationSelections};

use super::{
    ALLOCATION_RECOVERY_RULE_CATALOG, AllocationRecoveryRuleCatalogError,
    ORDERED_ALLOCATION_RECOVERY_RULES, ORDERED_SELECTED_LOWERING_RULES,
    RegisterAllocationRuleTargetApplicability, SELECTED_LOWERING_RULE_CATALOG,
    selected_allocation_recovery_rule, selected_lowering_rule_policy,
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
    assert_eq!(
        ALLOCATION_RECOVERY_RULE_CATALOG.map(|entry| entry.optimization()),
        ORDERED_ALLOCATION_RECOVERY_RULES,
    );
    assert!(ALLOCATION_RECOVERY_RULE_CATALOG.iter().all(|entry| {
        entry.payload().target() == RegisterAllocationRuleTargetApplicability::TargetIndependent
    }));
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
    assert_eq!(
        SELECTED_LOWERING_RULE_CATALOG.map(|entry| entry.optimization()),
        ORDERED_SELECTED_LOWERING_RULES,
    );
    assert!(SELECTED_LOWERING_RULE_CATALOG.iter().all(|entry| {
        entry.payload().target() == RegisterAllocationRuleTargetApplicability::TargetIndependent
    }));
    for optimization in ORDERED_SELECTED_LOWERING_RULES {
        let selections = OptimizationSelections::new([optimization]).unwrap();
        assert!(selected_lowering_rule_policy(&selections).is_ok());
    }
    let composition = OptimizationSelections::new(ORDERED_SELECTED_LOWERING_RULES).unwrap();
    assert!(selected_lowering_rule_policy(&composition).is_ok());
}
