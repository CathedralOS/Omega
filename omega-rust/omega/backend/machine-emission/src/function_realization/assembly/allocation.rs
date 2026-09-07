//! Allocation evidence roles bound by the existing realization receipt schemas.

use selected_instructions_to_register_homes::{AllocationEvidence, AllocationOutput};

use super::super::{error::FunctionRelativeOptimizationRealizationError, prelude::*};

pub(in crate::function_realization) fn selected_lowering_source<'source>(
    allocation: &'source AllocationOutput<'_>,
) -> Result<
    &'source StagedOptimizedPostSelectedLoweringHomeCustodyReceipt,
    FunctionRelativeOptimizationRealizationError,
> {
    match allocation.evidence() {
        AllocationEvidence::SelectedLowering(source) => Ok(source),
        _ => Err(FunctionRelativeOptimizationRealizationError::RootMismatch),
    }
}
