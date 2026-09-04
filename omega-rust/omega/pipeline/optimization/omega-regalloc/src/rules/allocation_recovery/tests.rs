use omega_optimization_core::{Optimization, OptimizationExecutionPhase, OptimizationSelections};

use super::{
    ALLOCATION_RECOVERY_RULE_CATALOG, AllocationRecoveryRuleCatalogError,
    ORDERED_ALLOCATION_RECOVERY_RULES, selected_allocation_recovery_rule,
};
use crate::RegisterAllocationRuleTargetApplicability;

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
