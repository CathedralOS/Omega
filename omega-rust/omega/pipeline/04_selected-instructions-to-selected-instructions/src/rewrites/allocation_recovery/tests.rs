use optimization_core::{Optimization, OptimizationExecutionPhase, OptimizationSelections};

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
        let phase = selections.project_phase(OptimizationExecutionPhase::AllocationRecovery);
        assert_eq!(
            selected_allocation_recovery_rule(&phase),
            Ok(Some(optimization))
        );
    }
    let composition = OptimizationSelections::new(ORDERED_ALLOCATION_RECOVERY_RULES).unwrap();
    let phase = composition.project_phase(OptimizationExecutionPhase::AllocationRecovery);
    assert_eq!(
        selected_allocation_recovery_rule(&phase),
        Err(AllocationRecoveryRuleCatalogError::UnsupportedComposition)
    );
}

/// The disabled-policy leg: an authored selection carrying no
/// allocation-recovery member projects an empty phase set, and the entrance
/// declines without naming a rule — the two catalog rules share this arm.
#[test]
fn unselected_allocation_recovery_phase_declines() {
    let selections = OptimizationSelections::new([Optimization::CopyPropagation]).unwrap();
    let phase = selections.project_phase(OptimizationExecutionPhase::AllocationRecovery);
    assert!(phase.is_empty());
    assert_eq!(selected_allocation_recovery_rule(&phase), Ok(None));
    let empty = OptimizationSelections::default();
    let phase = empty.project_phase(OptimizationExecutionPhase::AllocationRecovery);
    assert_eq!(selected_allocation_recovery_rule(&phase), Ok(None));
}
