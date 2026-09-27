use optimization_core::{Optimization, OptimizationExecutionPhase, OptimizationSelections};

use super::{
    ALLOCATION_RECOVERY_RULE_CATALOG, AllocationRecoveryRuleCatalogError,
    selected_allocation_recovery_rule,
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
    assert_eq!(
        declared,
        ALLOCATION_RECOVERY_RULE_CATALOG.map(|entry| entry.optimization())
    );
    assert!(ALLOCATION_RECOVERY_RULE_CATALOG.iter().all(|entry| {
        entry.payload().target() == RegisterAllocationRuleTargetApplicability::TargetIndependent
    }));
    for optimization in ALLOCATION_RECOVERY_RULE_CATALOG.map(|entry| entry.optimization()) {
        let selections = OptimizationSelections::new([optimization]).unwrap();
        let phase = selections.project_phase(OptimizationExecutionPhase::AllocationRecovery);
        assert_eq!(
            selected_allocation_recovery_rule(&phase),
            Ok(Some(optimization))
        );
    }
    let composition = OptimizationSelections::new(
        ALLOCATION_RECOVERY_RULE_CATALOG.map(|entry| entry.optimization()),
    )
    .unwrap();
    let phase = composition.project_phase(OptimizationExecutionPhase::AllocationRecovery);
    assert_eq!(
        selected_allocation_recovery_rule(&phase),
        Err(AllocationRecoveryRuleCatalogError::UnsupportedComposition)
    );
}
