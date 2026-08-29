use omega_optimization_core::{Optimization, OptimizationExecutionPhase, OptimizationSelections};

use super::{
    ORDERED_POST_ALLOCATION_MACHINE_RULES, PostAllocationMachineRuleCatalogError,
    selected_post_allocation_machine_rule,
};

#[test]
fn catalog_exactly_matches_the_post_allocation_machine_vocabulary() {
    let declared = Optimization::ALL
        .into_iter()
        .filter(|optimization| {
            optimization.execution_phase() == OptimizationExecutionPhase::PostAllocationMachine
        })
        .collect::<Vec<_>>();
    assert_eq!(declared, ORDERED_POST_ALLOCATION_MACHINE_RULES);
    for optimization in ORDERED_POST_ALLOCATION_MACHINE_RULES {
        let selections = OptimizationSelections::new([optimization]).unwrap();
        let (scheduled, phase) = selected_post_allocation_machine_rule(&selections).unwrap();
        assert_eq!(scheduled, optimization);
        assert_eq!(phase, selections);
    }
    assert!(matches!(
        selected_post_allocation_machine_rule(
            &OptimizationSelections::new(ORDERED_POST_ALLOCATION_MACHINE_RULES).unwrap()
        ),
        Err(PostAllocationMachineRuleCatalogError::UnsupportedComposition(_))
    ));
}
