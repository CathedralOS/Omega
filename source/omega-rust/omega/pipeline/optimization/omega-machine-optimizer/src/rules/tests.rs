use omega_optimization_core::{Optimization, OptimizationExecutionPhase, OptimizationSelections};
use omega_target::Architecture;

use super::{
    ORDERED_POST_ALLOCATION_MACHINE_RULES, POST_ALLOCATION_MACHINE_RULE_CATALOG,
    PostAllocationMachineRuleCatalogError, PostAllocationMachineRuleKind,
    selected_post_allocation_machine_rule,
};

#[test]
fn catalog_exactly_matches_the_post_allocation_machine_vocabulary() {
    assert_eq!(
        ORDERED_POST_ALLOCATION_MACHINE_RULES,
        [
            Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1,
            Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1,
            Optimization::X86SelectXorZeroI64MaterializationV1,
            Optimization::X86SelectMovR32Imm32ZeroExtendedI64MaterializationV1,
            Optimization::X86SelectMovR64Imm32SignExtendedI64MaterializationV1,
        ]
    );
    let declared = Optimization::ALL
        .into_iter()
        .filter(|optimization| {
            optimization.execution_phase() == OptimizationExecutionPhase::PostAllocationMachine
        })
        .collect::<Vec<_>>();
    assert_eq!(declared, ORDERED_POST_ALLOCATION_MACHINE_RULES);
    let expected_kinds = [
        PostAllocationMachineRuleKind::Aarch64Cbnz,
        PostAllocationMachineRuleKind::Aarch64Movn,
        PostAllocationMachineRuleKind::X86XorZero,
        PostAllocationMachineRuleKind::X86MovR32Imm32,
        PostAllocationMachineRuleKind::X86MovR64Imm32SignExtended,
    ];
    for (descriptor, expected_kind) in POST_ALLOCATION_MACHINE_RULE_CATALOG
        .into_iter()
        .zip(expected_kinds)
    {
        let optimization = descriptor.optimization();
        let architecture = descriptor.payload().architecture();
        assert_eq!(descriptor.payload().kind(), expected_kind);
        let selections = OptimizationSelections::new([optimization]).unwrap();
        let (scheduled, phase) =
            selected_post_allocation_machine_rule(&selections, architecture).unwrap();
        assert_eq!(scheduled, descriptor);
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
