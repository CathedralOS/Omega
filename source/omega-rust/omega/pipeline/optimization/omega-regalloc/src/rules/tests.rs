use omega_optimization_core::{Optimization, OptimizationExecutionPhase, OptimizationSelections};

use super::{
    ALLOCATION_RECOVERY_RULE_CATALOG, AllocationRecoveryRuleCatalogError, LiteralFoldPolicy,
    ORDERED_ALLOCATION_RECOVERY_RULES, ORDERED_SELECTED_LOWERING_RULES,
    RegisterAllocationRuleTargetApplicability, SELECTED_LOWERING_RULE_CATALOG,
    resolve_selected_lowering_rules, selected_allocation_recovery_rule,
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
    for entry in SELECTED_LOWERING_RULE_CATALOG {
        let selections = OptimizationSelections::new([entry.optimization()]).unwrap();
        let (selected, policy) = resolve_selected_lowering_rules(&selections).unwrap();
        assert_eq!(selected, selections);
        assert_eq!(policy, entry.payload().policy());
    }
    let composition = OptimizationSelections::new(ORDERED_SELECTED_LOWERING_RULES).unwrap();
    let (selected, policy) = resolve_selected_lowering_rules(&composition).unwrap();
    assert_eq!(selected.as_slice(), ORDERED_SELECTED_LOWERING_RULES);
    assert_eq!(
        policy,
        SELECTED_LOWERING_RULE_CATALOG
            .into_iter()
            .fold(LiteralFoldPolicy::empty(), |resolved, entry| {
                resolved.union(entry.payload().policy())
            })
    );
    assert!(policy.enables_exact_add());
    assert!(policy.enables_exact_subtract());
}
