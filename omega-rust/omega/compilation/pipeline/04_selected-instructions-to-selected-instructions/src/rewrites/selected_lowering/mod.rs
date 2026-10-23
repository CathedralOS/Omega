//! Optimizer module role: executable entrance. Selected-lowering rule entrance.
//!
//! [`SELECTED_LOWERING_RULE_CATALOG`] is the only enable/order declaration for
//! this phase. Catalog rows compose their exact payloads; the literal-fold
//! family owns proposal and independent replay.

mod catalog;
pub(crate) mod literal_fold;

use optimization_core::{
    OptimizationExecutionPhase, OptimizationPhaseSelections, OptimizationSelections,
};

pub(crate) use catalog::enabled_pair_rules;
pub(crate) use catalog::{
    SELECTED_LOWERING_RULE_CATALOG, SelectedLoweringRuleCatalogEntry,
    SelectedLoweringRuleCatalogError,
};
pub(crate) use literal_fold::{
    FunctionLiteralFold, LiteralFoldAction, LiteralFoldError, LiteralFoldPlan, LiteralFoldPolicy,
    LiteralFoldValidationReceipt, PairOperandShape, PairResultDisposition, PairUnitDefRelation,
    SelectedInstructionPairRule,
};
pub use literal_fold::{LiteralFoldIdentity, ValidatedLiteralFold};
pub(crate) use literal_fold::{fold_selected_incoming_literal, literal_fold_identity};

/// Resolve every selected-lowering catalog row in canonical catalog order.
///
/// The returned policy is a derived set of exact catalog payloads, never a
/// separately declared combined rule. Appending a row therefore requires an
/// explicit payload and cannot fall through an old whole-catalog special case.
pub(crate) fn resolve_selected_lowering_rules(
    selections: &OptimizationPhaseSelections,
) -> Result<(OptimizationSelections, LiteralFoldPolicy), SelectedLoweringRuleCatalogError> {
    let phase = super::required_stage_phase(
        selections,
        OptimizationExecutionPhase::SelectedLowering,
        SelectedLoweringRuleCatalogError::WrongPhase,
        || SelectedLoweringRuleCatalogError::MissingSelection,
        SelectedLoweringRuleCatalogError::UnsupportedSelection,
    )?;
    let policy = SELECTED_LOWERING_RULE_CATALOG
        .iter()
        .filter(|entry| phase.contains(entry.optimization()))
        .fold(LiteralFoldPolicy::empty(), |policy, entry| {
            policy.union(entry.payload().policy())
        });
    Ok((phase, policy))
}
