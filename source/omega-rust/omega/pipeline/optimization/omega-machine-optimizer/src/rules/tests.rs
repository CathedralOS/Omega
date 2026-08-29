use omega_optimization_core::{Optimization, OptimizationExecutionPhase, OptimizationSelections};
use omega_target::Architecture;

use super::{
    ORDERED_POST_ALLOCATION_MACHINE_RULES, POST_ALLOCATION_MACHINE_RULE_CATALOG,
    PostAllocationMachineRuleCatalogError, selected_post_allocation_machine_rule,
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
    for descriptor in POST_ALLOCATION_MACHINE_RULE_CATALOG {
        let optimization = descriptor.optimization();
        let architecture = *descriptor.payload();
        let selections = OptimizationSelections::new([optimization]).unwrap();
        let (scheduled, phase) =
            selected_post_allocation_machine_rule(&selections, architecture).unwrap();
        assert_eq!(scheduled, optimization);
        assert_eq!(phase, selections);

        let wrong = match architecture {
            Architecture::Aarch64 => Architecture::X86_64,
            Architecture::X86_64 => Architecture::Aarch64,
        };
        assert_eq!(
            selected_post_allocation_machine_rule(&selections, wrong),
            Err(PostAllocationMachineRuleCatalogError::UnsupportedTarget {
                optimization,
                required: architecture,
                actual: wrong,
            })
        );
    }
    assert!(matches!(
        selected_post_allocation_machine_rule(
            &OptimizationSelections::new(ORDERED_POST_ALLOCATION_MACHINE_RULES).unwrap(),
            Architecture::Aarch64,
        ),
        Err(PostAllocationMachineRuleCatalogError::UnsupportedComposition(_))
    ));
}
