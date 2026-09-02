use omega_machine_optimizer::{
    POST_ALLOCATION_MACHINE_RULE_CATALOG, PostAllocationMachineRuleCatalogEntry,
    PostAllocationMachineRuleCatalogError,
};
use omega_optimization_core::{Optimization, OptimizationExecutionPhase, OptimizationSelections};
use omega_psi_optimizer::built_in_psi_registries;
use omega_regalloc::AllocationRecoveryRuleCatalogError;
use omega_target::Architecture;

use crate::FunctionRelativeLayoutCatalogError;

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExpectedDisposition {
    Route(ResolvedPhysicalPhaseComposition),
    UnsupportedPhysicalComposition,
    AllocationRecoveryComposition,
    PostAllocationComposition(Optimization),
    PostAllocationTarget {
        optimization: Optimization,
        required: Architecture,
        actual: Architecture,
    },
    FunctionRelativeTarget {
        optimization: Optimization,
        required: Architecture,
        actual: Architecture,
    },
}

fn post_allocation_entry(optimization: Optimization) -> PostAllocationMachineRuleCatalogEntry {
    *POST_ALLOCATION_MACHINE_RULE_CATALOG
        .iter()
        .find(|entry| entry.optimization() == optimization)
        .expect("every post-allocation rule has one canonical catalog row")
}

fn expected_pair(
    first: Optimization,
    second: Optimization,
    architecture: Architecture,
) -> ExpectedDisposition {
    let pair = [first, second];
    let count = |phase| {
        pair.into_iter()
            .filter(|optimization| optimization.execution_phase() == phase)
            .count()
    };
    let selected_lowering = count(OptimizationExecutionPhase::SelectedLowering);
    let allocation_recovery = count(OptimizationExecutionPhase::AllocationRecovery);
    let post_allocation = count(OptimizationExecutionPhase::PostAllocationMachine);
    let function_relative = count(OptimizationExecutionPhase::FunctionRelativeLayout);

    if allocation_recovery > 1 {
        return ExpectedDisposition::AllocationRecoveryComposition;
    }
    if allocation_recovery == 1 {
        if selected_lowering + function_relative != 0 {
            return ExpectedDisposition::UnsupportedPhysicalComposition;
        }
        if post_allocation == 1 {
            let admitted = pair
                .contains(&Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1)
                && (pair.contains(&Optimization::X86SelectXorZeroI64MaterializationV1)
                    || pair.contains(
                        &Optimization::X86SelectMovR32Imm32ZeroExtendedI64MaterializationV1,
                    )
                    || pair.contains(
                        &Optimization::X86SelectMovR64Imm32SignExtendedI64MaterializationV1,
                    )
                    || pair.contains(
                        &Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1,
                    ));
            if !admitted {
                return ExpectedDisposition::UnsupportedPhysicalComposition;
            }
            let optimization = pair
                .into_iter()
                .find(|optimization| {
                    optimization.execution_phase()
                        == OptimizationExecutionPhase::PostAllocationMachine
                })
                .unwrap();
            let entry = post_allocation_entry(optimization);
            let required = entry.payload().architecture();
            if required != architecture {
                return ExpectedDisposition::PostAllocationTarget {
                    optimization,
                    required,
                    actual: architecture,
                };
            }
            return ExpectedDisposition::Route(
                ResolvedPhysicalPhaseComposition::AllocationRecovery {
                    rule: Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1,
                    post_allocation: Some(entry),
                },
            );
        }
    }
    if post_allocation > 1 {
        return ExpectedDisposition::PostAllocationComposition(first.min(second));
    }
    if post_allocation == 1 && function_relative == 1 {
        return ExpectedDisposition::UnsupportedPhysicalComposition;
    }

    let physical = pair.into_iter().find(|optimization| {
        optimization.execution_phase() == OptimizationExecutionPhase::PostAllocationMachine
    });
    if let Some(optimization) = physical {
        let entry = post_allocation_entry(optimization);
        let required = entry.payload().architecture();
        if required != architecture {
            return ExpectedDisposition::PostAllocationTarget {
                optimization,
                required,
                actual: architecture,
            };
        }
        return ExpectedDisposition::Route(ResolvedPhysicalPhaseComposition::NonAllocation(
            ResolvedNonAllocationComposition::PostAllocationMachine {
                entry,
                after_selected_lowering: selected_lowering != 0,
            },
        ));
    }

    if function_relative == 1 {
        let optimization = Optimization::X86RelaxConditionalBranchesToRel8V1;
        if architecture != Architecture::X86_64 {
            return ExpectedDisposition::FunctionRelativeTarget {
                optimization,
                required: Architecture::X86_64,
                actual: architecture,
            };
        }
        if selected_lowering != 0 {
            return ExpectedDisposition::Route(ResolvedPhysicalPhaseComposition::NonAllocation(
                ResolvedNonAllocationComposition::SelectedLoweringWithFunctionRelativeLayout,
            ));
        }
        return ExpectedDisposition::Route(ResolvedPhysicalPhaseComposition::NonAllocation(
            ResolvedNonAllocationComposition::FunctionRelativeLayout,
        ));
    }

    if allocation_recovery == 1 {
        let rule = pair
            .into_iter()
            .find(|optimization| {
                optimization.execution_phase() == OptimizationExecutionPhase::AllocationRecovery
            })
            .unwrap();
        return ExpectedDisposition::Route(ResolvedPhysicalPhaseComposition::AllocationRecovery {
            rule,
            post_allocation: None,
        });
    }
    if selected_lowering != 0 {
        return ExpectedDisposition::Route(ResolvedPhysicalPhaseComposition::NonAllocation(
            ResolvedNonAllocationComposition::SelectedLowering,
        ));
    }
    ExpectedDisposition::Route(ResolvedPhysicalPhaseComposition::NonAllocation(
        ResolvedNonAllocationComposition::Baseline,
    ))
}

fn actual_disposition(
    selections: &OptimizationSelections,
    architecture: Architecture,
) -> ExpectedDisposition {
    match resolve_physical_phase_composition(selections, architecture) {
        Ok(route) => ExpectedDisposition::Route(route),
        Err(OptimizedVerifiedPhysicalPipelineError::UnsupportedPhysicalPhaseComposition) => {
            ExpectedDisposition::UnsupportedPhysicalComposition
        }
        Err(OptimizedVerifiedPhysicalPipelineError::AllocationRecoveryRuleCatalog(
            AllocationRecoveryRuleCatalogError::UnsupportedComposition,
        )) => ExpectedDisposition::AllocationRecoveryComposition,
        Err(OptimizedVerifiedPhysicalPipelineError::PostAllocationMachineRuleCatalog(
            PostAllocationMachineRuleCatalogError::UnsupportedComposition(first),
        )) => ExpectedDisposition::PostAllocationComposition(first),
        Err(OptimizedVerifiedPhysicalPipelineError::PostAllocationMachineRuleCatalog(
            PostAllocationMachineRuleCatalogError::UnsupportedTarget {
                optimization,
                required,
                actual,
            },
        )) => ExpectedDisposition::PostAllocationTarget {
            optimization,
            required,
            actual,
        },
        Err(OptimizedVerifiedPhysicalPipelineError::FunctionRelativeLayoutRuleCatalog(
            FunctionRelativeLayoutCatalogError::UnsupportedTarget {
                optimization,
                required,
                actual,
            },
        )) => ExpectedDisposition::FunctionRelativeTarget {
            optimization,
            required,
            actual,
        },
        Err(error) => panic!("unexpected matrix error: {error:?}"),
    }
}

#[test]
fn every_exact_rule_pair_has_a_typed_physical_composition_disposition() {
    let mut accepted = 0;
    let mut unsupported = 0;
    let mut wrong_target = 0;
    let mut cells = 0;

    for (index, first) in Optimization::ALL.into_iter().enumerate() {
        for second in Optimization::ALL.into_iter().skip(index + 1) {
            let selections = OptimizationSelections::new([first, second]).unwrap();
            let expected_psi_passes = [first, second]
                .into_iter()
                .filter(|optimization| {
                    optimization.execution_phase() == OptimizationExecutionPhase::Psi
                })
                .count();
            assert_eq!(
                built_in_psi_registries(&selections).unwrap().len(),
                expected_psi_passes
            );
            let with_full_psi = OptimizationSelections::new(
                Optimization::ALL
                    .into_iter()
                    .filter(|optimization| {
                        optimization.execution_phase() == OptimizationExecutionPhase::Psi
                    })
                    .chain([first, second].into_iter().filter(|optimization| {
                        optimization.execution_phase() != OptimizationExecutionPhase::Psi
                    })),
            )
            .unwrap();
            assert_eq!(built_in_psi_registries(&with_full_psi).unwrap().len(), 6);
            for architecture in [Architecture::X86_64, Architecture::Aarch64] {
                let expected = expected_pair(first, second, architecture);
                let actual = actual_disposition(&selections, architecture);
                assert_eq!(
                    actual, expected,
                    "pair ({first:?}, {second:?}) on {architecture:?}"
                );
                assert_eq!(
                    actual_disposition(&with_full_psi, architecture),
                    actual,
                    "the complete Psi overlay changed ({first:?}, {second:?}) on {architecture:?}"
                );
                match actual {
                    ExpectedDisposition::Route(_) => accepted += 1,
                    ExpectedDisposition::PostAllocationTarget { .. }
                    | ExpectedDisposition::FunctionRelativeTarget { .. } => wrong_target += 1,
                    _ => unsupported += 1,
                }
                cells += 1;
            }
        }
    }

    assert_eq!(cells, 342);
    assert_eq!(accepted, 156);
    assert_eq!(unsupported, 110);
    assert_eq!(wrong_target, 76);
}

#[test]
fn selected_lowering_triples_have_explicit_supported_and_rejected_routes() {
    let selected = [
        Optimization::SelectedIncomingU12ExactAddImmediate,
        Optimization::SelectedIncomingU12ExactSubtractImmediate,
    ];
    for (machine, architecture) in [
        (
            Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1,
            Architecture::Aarch64,
        ),
        (
            Optimization::X86SelectXorZeroI64MaterializationV1,
            Architecture::X86_64,
        ),
    ] {
        let selections =
            OptimizationSelections::new(selected.into_iter().chain([machine])).unwrap();
        assert_eq!(
            resolve_physical_phase_composition(&selections, architecture).unwrap(),
            ResolvedPhysicalPhaseComposition::NonAllocation(
                ResolvedNonAllocationComposition::PostAllocationMachine {
                    entry: post_allocation_entry(machine),
                    after_selected_lowering: true,
                }
            )
        );
    }

    let with_layout = OptimizationSelections::new(
        selected
            .into_iter()
            .chain([Optimization::X86RelaxConditionalBranchesToRel8V1]),
    )
    .unwrap();
    assert_eq!(
        resolve_physical_phase_composition(&with_layout, Architecture::X86_64).unwrap(),
        ResolvedPhysicalPhaseComposition::NonAllocation(
            ResolvedNonAllocationComposition::SelectedLoweringWithFunctionRelativeLayout
        )
    );

    let machine_and_layout = OptimizationSelections::new([
        Optimization::SelectedIncomingU12ExactAddImmediate,
        Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1,
        Optimization::X86RelaxConditionalBranchesToRel8V1,
    ])
    .unwrap();
    assert!(matches!(
        resolve_physical_phase_composition(&machine_and_layout, Architecture::Aarch64),
        Err(OptimizedVerifiedPhysicalPipelineError::UnsupportedPhysicalPhaseComposition)
    ));
}
