use omega_machine_optimizer::{
    ORDERED_POST_ALLOCATION_MACHINE_RULES, POST_ALLOCATION_MACHINE_RULE_CATALOG,
    PostAllocationMachineRuleCatalogError,
};
use omega_optimization_core::{OptimizationSelections, PostTerminalOptimizationSelections};
use omega_target::Architecture;

use crate::coordination::physical_pipeline::{
    OptimizedVerifiedPhysicalPipelineError, PhysicalOptimizationPhaseSelections,
    resolve_physical_phase_composition,
};

#[test]
fn every_machine_rule_rejects_the_wrong_architecture_before_execution() {
    for rule in ORDERED_POST_ALLOCATION_MACHINE_RULES {
        let required = POST_ALLOCATION_MACHINE_RULE_CATALOG
            .iter()
            .find(|descriptor| descriptor.optimization() == rule)
            .unwrap()
            .payload()
            .architecture();
        let actual = match required {
            Architecture::Aarch64 => Architecture::X86_64,
            Architecture::X86_64 => Architecture::Aarch64,
        };
        let selections = OptimizationSelections::new([rule]).unwrap();
        let selections = PostTerminalOptimizationSelections::new(selections).unwrap();
        let selections = PhysicalOptimizationPhaseSelections::project(&selections).unwrap();
        assert!(matches!(
            resolve_physical_phase_composition(&selections, actual),
            Err(
                OptimizedVerifiedPhysicalPipelineError::PostAllocationMachineRuleCatalog(
                    PostAllocationMachineRuleCatalogError::UnsupportedTarget {
                        optimization,
                        required: rejected_required,
                        actual: rejected_actual,
                    }
                )
            ) if optimization == rule
                && rejected_required == required
                && rejected_actual == actual
        ));
    }
}
