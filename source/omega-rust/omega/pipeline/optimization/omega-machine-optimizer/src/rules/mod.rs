//! Optimizer module role: executable entrance. Post-allocation machine-rule entrance.
//!
//! [`POST_ALLOCATION_MACHINE_RULE_CATALOG`] is the only enable/order and target
//! applicability declaration. This entrance projects exact selections through
//! it; target folders then own proposal and independent validation.

pub mod aarch64;
pub mod x86_64;

mod catalog;

#[cfg(test)]
mod tests;

use omega_optimization_core::{Optimization, OptimizationExecutionPhase, OptimizationSelections};
use omega_target::Architecture;

pub use catalog::{
    ORDERED_POST_ALLOCATION_MACHINE_RULES, POST_ALLOCATION_MACHINE_RULE_CATALOG,
    PostAllocationMachineRuleCatalogEntry, PostAllocationMachineRuleCatalogError,
};

/// Select the single post-allocation machine rule currently admitted by the
/// physical pipeline, preserving the exact phase-selection identity.
pub fn selected_post_allocation_machine_rule(
    selections: &OptimizationSelections,
    architecture: Architecture,
) -> Result<(Optimization, OptimizationSelections), PostAllocationMachineRuleCatalogError> {
    let phase = selections.for_phase(OptimizationExecutionPhase::PostAllocationMachine);
    match phase.as_slice() {
        [selected] => {
            let Some(descriptor) = POST_ALLOCATION_MACHINE_RULE_CATALOG
                .iter()
                .find(|descriptor| descriptor.optimization() == *selected)
            else {
                return Err(PostAllocationMachineRuleCatalogError::UnsupportedSelection(
                    *selected,
                ));
            };
            let required = *descriptor.payload();
            if required != architecture {
                return Err(PostAllocationMachineRuleCatalogError::UnsupportedTarget {
                    optimization: *selected,
                    required,
                    actual: architecture,
                });
            }
            Ok((*selected, phase))
        }
        [] => Err(PostAllocationMachineRuleCatalogError::MissingSelection),
        selected => Err(PostAllocationMachineRuleCatalogError::UnsupportedComposition(selected[0])),
    }
}

/// Require one exact post-allocation rule at a rule-specific custody join.
pub fn require_post_allocation_machine_rule(
    selections: &OptimizationSelections,
    expected: Optimization,
    architecture: Architecture,
) -> Result<OptimizationSelections, PostAllocationMachineRuleCatalogError> {
    let (selected, phase) = selected_post_allocation_machine_rule(selections, architecture)?;
    if selected != expected {
        return Err(PostAllocationMachineRuleCatalogError::UnsupportedSelection(
            selected,
        ));
    }
    Ok(phase)
}
