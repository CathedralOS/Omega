use std::sync::Arc;

use selected_instructions::{SelectedFunction, SelectedInstructionId, SelectedInstructionPlan};
use target_operations_to_selected_instructions::selected_instruction_plan_identity;

use super::{AddressFoldError, AddressFoldReceipt, ValidatedAddressFold, admission};
use crate::ValidatedSelectedAnalysis;

/// The measured-step contract the executing pass charges per evaluated
/// candidate: the shared plan scan plus the backward producer-location scan
/// and the base-redefinition interval scan, each bounded by the function's
/// largest block. Admission and validation meter the same scans at the
/// actual consumer's block, so this bound is exact whenever the candidates
/// sit in the function's largest block.
pub(crate) fn measured_steps(
    plan: &SelectedInstructionPlan,
    function: &SelectedFunction,
) -> Result<u64, AddressFoldError> {
    let largest_block = function
        .blocks
        .iter()
        .map(|block| block.instructions.len())
        .max()
        .unwrap_or(0);
    let steps = admission::plan_scan_steps(plan)?
        .checked_add(largest_block)
        .and_then(|total| total.checked_add(largest_block))
        .ok_or(AddressFoldError::IdentityOverflow)?;
    u64::try_from(steps).map_err(|_| AddressFoldError::IdentityOverflow)
}

/// Admit a freshly produced plan with its receipt; the producer calls this
/// instead of replaying its own output.
pub(crate) fn seal_address_fold(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    access: SelectedInstructionId,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedAddressFold, AddressFoldError> {
    Ok(ValidatedAddressFold {
        receipt: AddressFoldReceipt {
            source_selected: source.selected_identity(),
            transformed_selected: selected_instruction_plan_identity(&proposed),
            optimization_unit: source.optimization_unit_identity(),
            fuel_schedule: source.fuel_schedule_identity(),
            function_index,
            access,
        },
        transformed: Arc::new(proposed),
    })
}
