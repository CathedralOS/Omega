//! Post-allocation machine-rule entrance.
//!
//! [`ORDERED_POST_ALLOCATION_MACHINE_RULES`] is the only enable/order table.
//! This entrance projects exact selections through that table; target folders
//! then own proposal and independent validation for each named rule.

pub mod aarch64;
pub mod x86_64;

mod catalog;

#[cfg(test)]
mod tests;

use omega_optimization_core::{Optimization, OptimizationExecutionPhase, OptimizationSelections};

pub use catalog::{ORDERED_POST_ALLOCATION_MACHINE_RULES, PostAllocationMachineRuleCatalogError};

/// Select the single post-allocation machine rule currently admitted by the
/// physical pipeline, preserving the exact phase-selection identity.
pub fn selected_post_allocation_machine_rule(
    selections: &OptimizationSelections,
) -> Result<(Optimization, OptimizationSelections), PostAllocationMachineRuleCatalogError> {
    let phase = selections.for_phase(OptimizationExecutionPhase::PostAllocationMachine);
    match phase.as_slice() {
        [selected] if ORDERED_POST_ALLOCATION_MACHINE_RULES.contains(selected) => {
            Ok((*selected, phase))
        }
        [] => Err(PostAllocationMachineRuleCatalogError::MissingSelection),
        [selected] => Err(PostAllocationMachineRuleCatalogError::UnsupportedSelection(
            *selected,
        )),
        selected => Err(PostAllocationMachineRuleCatalogError::UnsupportedComposition(selected[0])),
    }
}

/// Require one exact post-allocation rule at a rule-specific custody join.
pub fn require_post_allocation_machine_rule(
    selections: &OptimizationSelections,
    expected: Optimization,
) -> Result<OptimizationSelections, PostAllocationMachineRuleCatalogError> {
    let (selected, phase) = selected_post_allocation_machine_rule(selections)?;
    if selected != expected {
        return Err(PostAllocationMachineRuleCatalogError::UnsupportedSelection(
            selected,
        ));
    }
    Ok(phase)
}
