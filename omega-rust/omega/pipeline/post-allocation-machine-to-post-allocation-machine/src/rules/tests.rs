use optimization_core::{Optimization, OptimizationExecutionPhase, OptimizationSelections};
use target::Architecture;

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
            Optimization::Aarch64ElideSameViewCopyI64BeforeReturnV1,
            Optimization::Aarch64ElideSameViewCopyI64BeforeCompareZeroV1,
            Optimization::Aarch64ElideSameViewCopyI64BeforeCompareI64LeftOperandV1,
            Optimization::Aarch64ElideSameViewCopyI64BeforeCompareI64RightOperandV1,
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
        PostAllocationMachineRuleKind::Aarch64SameViewCopyElision,
        PostAllocationMachineRuleKind::Aarch64SameViewCopyBeforeCompareZeroElision,
        PostAllocationMachineRuleKind::Aarch64SameViewCopyBeforeCompareI64LeftOperandElision,
        PostAllocationMachineRuleKind::Aarch64SameViewCopyBeforeCompareI64RightOperandElision,
    ];
    assert_eq!(
        POST_ALLOCATION_MACHINE_RULE_CATALOG.len(),
        expected_kinds.len()
    );
    for (descriptor, expected_kind) in POST_ALLOCATION_MACHINE_RULE_CATALOG
        .into_iter()
        .zip(expected_kinds)
    {
        let optimization = descriptor.optimization();
        let architecture = descriptor.payload().architecture();
        assert_eq!(descriptor.payload().kind(), expected_kind);
        let selections = OptimizationSelections::new([optimization]).unwrap();
        let phase = selections.project_phase(OptimizationExecutionPhase::PostAllocationMachine);
        let (scheduled, phase) =
            selected_post_allocation_machine_rule(&phase, architecture).unwrap();
        assert_eq!(scheduled, descriptor);
        assert_eq!(phase, selections);

        let wrong = match architecture {
            Architecture::Aarch64 => Architecture::X86_64,
            Architecture::X86_64 => Architecture::Aarch64,
        };
        assert_eq!(
            selected_post_allocation_machine_rule(
                &selections.project_phase(OptimizationExecutionPhase::PostAllocationMachine),
                wrong,
            ),
            Err(PostAllocationMachineRuleCatalogError::UnsupportedTarget {
                optimization,
                required: architecture,
                actual: wrong,
            })
        );
    }
    let composition = OptimizationSelections::new(ORDERED_POST_ALLOCATION_MACHINE_RULES).unwrap();
    let phase = composition.project_phase(OptimizationExecutionPhase::PostAllocationMachine);
    assert!(matches!(
        selected_post_allocation_machine_rule(&phase, Architecture::Aarch64),
        Err(PostAllocationMachineRuleCatalogError::UnsupportedComposition(_))
    ));
}
