//! Optimizer module role: executable entrance. Allocation-recovery rule entrance.
//!
//! [`ALLOCATION_RECOVERY_RULE_CATALOG`] is the only enable/order declaration
//! for this phase. This entrance admits its current single-rule execution
//! contract; named rule folders own proposal and independent replay.

mod catalog;
pub(crate) mod fixed_view_copy;
pub(crate) mod pressure_rematerialization;

use optimization_core::{Optimization, OptimizationExecutionPhase, OptimizationPhaseSelections};

use super::catalog::selected_stage_catalog_contains;

pub use catalog::AllocationRecoveryRuleCatalogError;
pub(crate) use catalog::{ALLOCATION_RECOVERY_RULE_CATALOG, AllocationRecoveryRuleCatalogEntry};
pub(crate) use fixed_view_copy::materialize_fixed_view_copies;
pub use fixed_view_copy::{
    FixedViewCopy, FixedViewCopyDestination, FixedViewCopyPlan, FixedViewCopyPolicy,
    FixedViewCopySourceEvidence, ValidatedFixedViewCopies, fixed_view_copy_identity,
};
pub(crate) use fixed_view_copy::{
    FixedViewCopyError, FixedViewCopyValidationReceipt,
};
pub(crate) use pressure_rematerialization::pressure_rematerialization_identity;
pub(crate) use pressure_rematerialization::{
    FunctionPressureRematerialization, PressureRematerializationAction,
    PressureRematerializationPlan, PressureRematerializationRewrite,
    PressureRematerializationValidationReceipt,
};
pub use pressure_rematerialization::{
    PressureRematerializationError, PressureRematerializationPolicy,
    ValidatedPressureRematerialization, rematerialize_selected_active_resident,
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
