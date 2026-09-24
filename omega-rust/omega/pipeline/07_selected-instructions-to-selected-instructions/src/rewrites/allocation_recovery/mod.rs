//! Optimizer module role: executable entrance. Allocation-recovery rule entrance.
//!
//! [`ALLOCATION_RECOVERY_RULE_CATALOG`] is the only enable/order declaration
//! for this phase. This entrance admits its current single-rule execution
//! contract; named rule folders own proposal and independent replay.

mod catalog;
pub(crate) mod fixed_view_copy;
pub(crate) mod pressure_rematerialization;

#[cfg(test)]
mod tests;

use optimization_core::{Optimization, OptimizationExecutionPhase, OptimizationPhaseSelections};

use super::catalog::selected_stage_catalog_contains;

pub use catalog::{
    ALLOCATION_RECOVERY_RULE_CATALOG, AllocationRecoveryRuleCatalogEntry,
    AllocationRecoveryRuleCatalogError, AllocationRecoveryRuleCatalogPayload,
    ORDERED_ALLOCATION_RECOVERY_RULES,
};
pub(crate) use fixed_view_copy::materialize_fixed_view_copies;
pub use fixed_view_copy::{
    FixedViewCopy, FixedViewCopyDecodeError, FixedViewCopyDestination, FixedViewCopyError,
    FixedViewCopyPlan, FixedViewCopyPolicy, FixedViewCopySourceEvidence,
    FixedViewCopyValidationReceipt, ValidatedFixedViewCopies, fixed_view_copy_identity,
    validate_fixed_view_copies,
};
pub(crate) use pressure_rematerialization::pressure_rematerialization_identity;
pub use pressure_rematerialization::{
    FunctionPressureRematerialization, PressureRematerializationAction,
    PressureRematerializationDecodeError, PressureRematerializationError,
    PressureRematerializationPlan, PressureRematerializationPolicy,
    PressureRematerializationRewrite, PressureRematerializationValidationReceipt,
    ValidatedPressureRematerialization, rematerialize_selected_active_resident,
    validate_pressure_rematerialization,
};

/// Select the single allocation-recovery rule currently admitted by the
/// physical pipeline. Empty phase selections deliberately return `None`.
pub fn selected_allocation_recovery_rule(
    selections: &OptimizationPhaseSelections,
) -> Result<Option<Optimization>, AllocationRecoveryRuleCatalogError> {
    let phase = selections
        .require_phase(OptimizationExecutionPhase::AllocationRecovery)
        .map_err(AllocationRecoveryRuleCatalogError::WrongPhase)?;
    match phase.as_slice() {
        [] => Ok(None),
        [selected]
            if selected_stage_catalog_contains(
                OptimizationExecutionPhase::AllocationRecovery,
                *selected,
            ) =>
        {
            Ok(Some(*selected))
        }
        [unsupported] => Err(AllocationRecoveryRuleCatalogError::UnsupportedSelection(
            *unsupported,
        )),
        _ => Err(AllocationRecoveryRuleCatalogError::UnsupportedComposition),
    }
}
