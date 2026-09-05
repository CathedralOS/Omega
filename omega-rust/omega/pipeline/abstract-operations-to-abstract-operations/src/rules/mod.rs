//! Optimizer module role: executable entrance. Built-in Psi optimization stage entrance.
//!
//! [`PSI_PASS_CATALOG`] is the only enable/disable and pass-order table. This
//! entrance filters exact selections through it and returns one ordered
//! registry per selected pass. Descend through `passes/<exact-pass>/mod.rs`
//! for that pass's visible local rule order, then into named rule mechanics.
//! Independent acceptance remains in `optimization-validation`.

mod catalog;
mod passes;

use optimization::PsiOptimizationSelections;
use optimization_core::OptimizationSelections;

use crate::{OrderedRuleRegistry, RuleRegistryError};

pub use catalog::{
    ORDERED_PSI_PASSES, PSI_PASS_CATALOG, PsiPassCatalogEntry, PsiPassTargetApplicability,
};
pub use passes::*;

pub fn built_in_psi_registry(
    selections: &OptimizationSelections,
) -> Result<OrderedRuleRegistry, RuleRegistryError> {
    let projection = selections.project_psi();
    built_in_psi_registry_for_selections(projection.selections())
}

/// Construct at most one Psi pass registry from Psi's own target-neutral
/// selection vocabulary. The unified-build entrance above is a migration
/// adapter and performs only the exhaustive structural projection.
pub fn built_in_psi_registry_for_selections(
    selections: &PsiOptimizationSelections,
) -> Result<OrderedRuleRegistry, RuleRegistryError> {
    let mut registries = built_in_psi_registries_for_selections(selections)?;
    if registries.len() > 1 {
        return Err(RuleRegistryError::UnsupportedOptimizationCombination);
    }
    Ok(registries
        .pop()
        .unwrap_or_else(|| OrderedRuleRegistry::new(Vec::new()).expect("empty registry is valid")))
}

/// Resolve exact selections in canonical catalog order.
pub fn built_in_psi_registries(
    selections: &OptimizationSelections,
) -> Result<Vec<OrderedRuleRegistry>, RuleRegistryError> {
    let projection = selections.project_psi();
    built_in_psi_registries_for_selections(projection.selections())
}

/// Resolve exact Psi-owned selections in canonical catalog order.
pub fn built_in_psi_registries_for_selections(
    selections: &PsiOptimizationSelections,
) -> Result<Vec<OrderedRuleRegistry>, RuleRegistryError> {
    if let Some(unsupported) = selections.as_slice().iter().find(|optimization| {
        !PSI_PASS_CATALOG
            .iter()
            .any(|entry| entry.optimization() == **optimization)
    }) {
        return Err(RuleRegistryError::UnsupportedOptimization(*unsupported));
    }

    PSI_PASS_CATALOG
        .iter()
        .copied()
        .filter(|entry| selections.contains(entry.optimization()))
        .map(|entry| catalog::registry_for_optimization(entry.optimization()))
        .collect()
}
