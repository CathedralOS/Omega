//! Optimizer module role: executable entrance. Selected-lowering rule entrance.
//!
//! [`SELECTED_LOWERING_RULE_CATALOG`] is the only enable/order declaration for
//! this phase. Catalog rows compose their exact payloads; the literal-fold
//! family owns proposal and independent replay.

mod catalog;
pub(crate) mod literal_fold;

#[cfg(test)]
mod tests;

use omega_optimization_core::{
    OptimizationExecutionPhase, OptimizationPhaseSelections, OptimizationSelections,
};

pub use catalog::*;
pub use literal_fold::*;

/// Resolve every selected-lowering catalog row in canonical catalog order.
///
/// The returned policy is a derived set of exact catalog payloads, never a
/// separately declared combined rule. Appending a row therefore requires an
/// explicit payload and cannot fall through an old whole-catalog special case.
pub fn resolve_selected_lowering_rules(
    selections: &OptimizationPhaseSelections,
) -> Result<(OptimizationSelections, LiteralFoldPolicy), SelectedLoweringRuleCatalogError> {
    let phase = selections
        .require_phase(OptimizationExecutionPhase::SelectedLowering)
        .map_err(SelectedLoweringRuleCatalogError::WrongPhase)?;
    if phase.is_empty() {
        return Err(SelectedLoweringRuleCatalogError::MissingSelection);
    }
    if let Some(unsupported) = phase.as_slice().iter().find(|selected| {
        !SELECTED_LOWERING_RULE_CATALOG
            .iter()
            .any(|entry| entry.optimization() == **selected)
    }) {
        return Err(SelectedLoweringRuleCatalogError::UnsupportedSelection(
            *unsupported,
        ));
    }
    let policy = SELECTED_LOWERING_RULE_CATALOG
        .iter()
        .filter(|entry| phase.contains(entry.optimization()))
        .fold(LiteralFoldPolicy::empty(), |policy, entry| {
            policy.union(entry.payload().policy())
        });
    Ok((phase.clone(), policy))
}
