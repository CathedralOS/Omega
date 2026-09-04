use omega_abstract_operations_optimizer::{
    PSI_PASS_CATALOG, PsiPassTargetApplicability, built_in_psi_registries,
};
use omega_machine_optimizer::{
    POST_ALLOCATION_MACHINE_RULE_CATALOG, PostAllocationMachineRuleCatalogError,
    selected_post_allocation_machine_rule,
};
use omega_optimization_core::{Optimization, OptimizationExecutionPhase, OptimizationSelections};
use omega_regalloc::{
    ALLOCATION_RECOVERY_RULE_CATALOG, AllocationRecoveryRuleCatalogError,
    RegisterAllocationRuleTargetApplicability, SELECTED_LOWERING_RULE_CATALOG,
    SelectedLoweringRuleCatalogError, resolve_selected_lowering_rules,
    selected_allocation_recovery_rule,
};
use omega_target::{Architecture, NativeTarget};

use crate::{
    FUNCTION_RELATIVE_LAYOUT_RULE_CATALOG, FunctionRelativeLayoutCatalogError,
    stages::layout::x86_branch_relaxation::x86_rel8_selected,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TestTargetDisposition {
    TargetIndependent,
    Architecture(Architecture),
}

#[test]
fn every_physical_catalog_rejects_a_projection_for_another_phase() {
    let selections = OptimizationSelections::new([Optimization::ControlFlowCleanup]).unwrap();
    let psi = selections.project_phase(OptimizationExecutionPhase::Psi);

    assert!(matches!(
        resolve_selected_lowering_rules(&psi),
        Err(SelectedLoweringRuleCatalogError::WrongPhase(_))
    ));
    assert!(matches!(
        selected_allocation_recovery_rule(&psi),
        Err(AllocationRecoveryRuleCatalogError::WrongPhase(_))
    ));
    assert!(matches!(
        selected_post_allocation_machine_rule(&psi, Architecture::X86_64),
        Err(PostAllocationMachineRuleCatalogError::WrongPhase(_))
    ));
    assert!(matches!(
        x86_rel8_selected(&psi, Architecture::X86_64),
        Err(FunctionRelativeLayoutCatalogError::WrongPhase(_))
    ));
}

fn declared_catalog() -> Vec<(
    Optimization,
    OptimizationExecutionPhase,
    TestTargetDisposition,
)> {
    PSI_PASS_CATALOG
        .iter()
        .map(|entry| {
            assert_eq!(
                entry.target_applicability(),
                PsiPassTargetApplicability::TargetIndependent
            );
            (
                Optimization::from(entry.optimization()),
                OptimizationExecutionPhase::Psi,
                TestTargetDisposition::TargetIndependent,
            )
        })
        .chain(SELECTED_LOWERING_RULE_CATALOG.iter().map(|entry| {
            assert_eq!(
                entry.payload().target(),
                RegisterAllocationRuleTargetApplicability::TargetIndependent
            );
            (
                entry.optimization(),
                OptimizationExecutionPhase::SelectedLowering,
                TestTargetDisposition::TargetIndependent,
            )
        }))
        .chain(ALLOCATION_RECOVERY_RULE_CATALOG.iter().map(|entry| {
            assert_eq!(
                entry.payload().target(),
                RegisterAllocationRuleTargetApplicability::TargetIndependent
            );
            (
                entry.optimization(),
                OptimizationExecutionPhase::AllocationRecovery,
                TestTargetDisposition::TargetIndependent,
            )
        }))
        .chain(POST_ALLOCATION_MACHINE_RULE_CATALOG.iter().map(|entry| {
            (
                entry.optimization(),
                OptimizationExecutionPhase::PostAllocationMachine,
                TestTargetDisposition::Architecture(entry.payload().architecture()),
            )
        }))
        .chain(FUNCTION_RELATIVE_LAYOUT_RULE_CATALOG.iter().map(|entry| {
            (
                entry.optimization(),
                OptimizationExecutionPhase::FunctionRelativeLayout,
                TestTargetDisposition::Architecture(*entry.payload()),
            )
        }))
        .collect()
}

#[test]
fn every_exact_optimization_has_one_rule_stage_disposition() {
    let catalog = declared_catalog();

    for optimization in Optimization::ALL {
        let dispositions = catalog
            .iter()
            .filter(|(scheduled, _, _)| *scheduled == optimization)
            .map(|(scheduled, phase, _)| (*phase, *scheduled))
            .collect::<Vec<_>>();
        assert_eq!(
            dispositions,
            vec![(optimization.execution_phase(), optimization)],
            "{optimization:?} must occur once in its declared phase catalog"
        );
    }

    assert_eq!(catalog.len(), Optimization::ALL.len());
    assert_eq!(
        catalog
            .iter()
            .filter(|(_, _, target)| *target == TestTargetDisposition::TargetIndependent)
            .count(),
        10
    );
    assert_eq!(
        catalog
            .iter()
            .filter(|(_, _, target)| {
                *target == TestTargetDisposition::Architecture(Architecture::Aarch64)
            })
            .count(),
        6
    );
    assert_eq!(
        catalog
            .iter()
            .filter(|(_, _, target)| {
                *target == TestTargetDisposition::Architecture(Architecture::X86_64)
            })
            .count(),
        4
    );
}

#[test]
fn every_exact_optimization_has_an_exhaustive_named_target_disposition() {
    let targets = [
        ("linux-x64", NativeTarget::linux_x64()),
        ("windows-x64", NativeTarget::windows_x64()),
        ("uefi-x64", NativeTarget::uefi_x64()),
        ("linux-arm64", NativeTarget::linux_arm64()),
        ("macos-arm64", NativeTarget::macos_arm64()),
    ];
    let mut scheduled = 0;
    let mut rejected = 0;

    for (optimization, phase, disposition) in declared_catalog() {
        let selections = OptimizationSelections::new([optimization]).unwrap();
        let phase_selections = selections.project_phase(phase);
        for (target_name, target) in targets {
            let applicable = match disposition {
                TestTargetDisposition::TargetIndependent => true,
                TestTargetDisposition::Architecture(required) => target.architecture == required,
            };
            match (phase, applicable) {
                (OptimizationExecutionPhase::Psi, true) => {
                    assert_eq!(built_in_psi_registries(&selections).unwrap().len(), 1);
                }
                (OptimizationExecutionPhase::SelectedLowering, true) => {
                    resolve_selected_lowering_rules(&phase_selections).unwrap();
                }
                (OptimizationExecutionPhase::AllocationRecovery, true) => {
                    assert_eq!(
                        selected_allocation_recovery_rule(&phase_selections),
                        Ok(Some(optimization))
                    );
                }
                (OptimizationExecutionPhase::PostAllocationMachine, true) => {
                    assert_eq!(
                        selected_post_allocation_machine_rule(
                            &phase_selections,
                            target.architecture
                        )
                        .unwrap()
                        .0,
                        *POST_ALLOCATION_MACHINE_RULE_CATALOG
                            .iter()
                            .find(|entry| entry.optimization() == optimization)
                            .unwrap()
                    );
                }
                (OptimizationExecutionPhase::FunctionRelativeLayout, true) => {
                    assert_eq!(
                        x86_rel8_selected(&phase_selections, target.architecture),
                        Ok(true)
                    );
                }
                (OptimizationExecutionPhase::PostAllocationMachine, false) => {
                    let TestTargetDisposition::Architecture(required) = disposition else {
                        unreachable!()
                    };
                    assert_eq!(
                        selected_post_allocation_machine_rule(
                            &phase_selections,
                            target.architecture,
                        ),
                        Err(PostAllocationMachineRuleCatalogError::UnsupportedTarget {
                            optimization,
                            required,
                            actual: target.architecture,
                        }),
                        "{optimization:?} must reject {target_name} by name"
                    );
                }
                (OptimizationExecutionPhase::FunctionRelativeLayout, false) => {
                    let TestTargetDisposition::Architecture(required) = disposition else {
                        unreachable!()
                    };
                    assert_eq!(
                        x86_rel8_selected(&phase_selections, target.architecture),
                        Err(FunctionRelativeLayoutCatalogError::UnsupportedTarget {
                            optimization,
                            required,
                            actual: target.architecture,
                        }),
                        "{optimization:?} must reject {target_name} by name"
                    );
                }
                _ => panic!("invalid catalog disposition for {optimization:?}"),
            }
            if applicable {
                scheduled += 1;
            } else {
                rejected += 1;
            }
        }
    }

    assert_eq!(scheduled, 74);
    assert_eq!(rejected, 26);
}
