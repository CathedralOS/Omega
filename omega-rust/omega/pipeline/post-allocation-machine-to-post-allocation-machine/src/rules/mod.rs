//! Optimizer module role: executable entrance. Post-allocation machine-rule entrance.
//!
//! [`POST_ALLOCATION_MACHINE_RULE_CATALOG`] is the only enable/order and target
//! applicability declaration. This entrance projects exact selections through
//! it; target folders then own proposal and independent validation.

pub mod aarch64;
pub mod x86_64;

mod catalog;
mod peephole_matching;

#[cfg(test)]
mod tests;

use optimization_core::{
    Optimization, OptimizationExecutionPhase, OptimizationPhaseSelections, OptimizationSelections,
};
use target::Architecture;

pub use catalog::{
    ORDERED_POST_ALLOCATION_MACHINE_RULES, POST_ALLOCATION_MACHINE_RULE_CATALOG,
    PostAllocationMachineRuleCatalogEntry, PostAllocationMachineRuleCatalogError,
    PostAllocationMachineRuleCatalogPayload, PostAllocationMachineRuleKind,
};

/// Select the single post-allocation machine rule currently admitted by the
/// physical pipeline, preserving the exact phase-selection identity.
pub fn selected_post_allocation_machine_rule(
    selections: &OptimizationPhaseSelections,
    architecture: Architecture,
) -> Result<
    (
        PostAllocationMachineRuleCatalogEntry,
        OptimizationSelections,
    ),
    PostAllocationMachineRuleCatalogError,
> {
    let phase = selections
        .require_phase(OptimizationExecutionPhase::PostAllocationMachine)
        .map_err(PostAllocationMachineRuleCatalogError::WrongPhase)?;
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
            let required = descriptor.payload().architecture();
            if required != architecture {
                return Err(PostAllocationMachineRuleCatalogError::UnsupportedTarget {
                    optimization: *selected,
                    required,
                    actual: architecture,
                });
            }
            Ok((*descriptor, phase.clone()))
        }
        [] => Err(PostAllocationMachineRuleCatalogError::MissingSelection),
        selected => Err(PostAllocationMachineRuleCatalogError::UnsupportedComposition(selected[0])),
    }
}

/// Require one exact post-allocation rule at a rule-specific custody join.
pub fn require_post_allocation_machine_rule(
    selections: &OptimizationPhaseSelections,
    expected: Optimization,
    architecture: Architecture,
) -> Result<OptimizationSelections, PostAllocationMachineRuleCatalogError> {
    let (entry, phase) = selected_post_allocation_machine_rule(selections, architecture)?;
    if entry.optimization() != expected {
        return Err(PostAllocationMachineRuleCatalogError::UnsupportedSelection(
            entry.optimization(),
        ));
    }
    Ok(phase)
}
