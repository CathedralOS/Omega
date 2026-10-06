//! Independent validation of store-to-load forwarding.
//!
//! The validator never calls [`super::admission`]: it re-derives the
//! forwarding's legality from the source records — the named instruction's
//! load kind against its single read row, the byte extent the row's place and
//! range spell (or the fixed byte a resolved sequence index lands it on), the
//! backward walk that must find one covering store of one register on every
//! path, the crossed-edge and walked-span audits that keep that register and
//! the load's result identical, and the target's own copy row — then rebuilds
//! the function the contract demands and requires the proposal to equal it.
//! Restoring the load and its read row must reproduce the complete source by
//! content. A producer admission error therefore fails validation even when
//! the proposal is exactly what that producer emitted.

use std::sync::Arc;

use selected_instructions::{SelectedInstructionId, SelectedInstructionPlan};
use target_operations_to_selected_instructions::selected_instruction_plan_identity;

use super::{
    StoredLoadForwardingError, StoredLoadForwardingReceipt, ValidatedStoredLoadForwarding,
};
use crate::ValidatedSelectedAnalysis;

/// Admit a freshly produced plan with its receipt; the producer calls this
/// instead of replaying its own output.
pub(crate) fn seal_stored_load_forwarding(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    load: SelectedInstructionId,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedStoredLoadForwarding, StoredLoadForwardingError> {
    let transformed_selected = selected_instruction_plan_identity(&proposed);
    let mut canonical = b"omega.stored-load-forwarding.v1".to_vec();
    canonical.extend_from_slice(&source.selected_identity().bytes());
    canonical.extend_from_slice(&transformed_selected.bytes());
    canonical.extend_from_slice(&source.optimization_unit_identity().bytes());
    canonical.extend_from_slice(&source.fuel_schedule_identity().marker().to_le_bytes());
    canonical.extend_from_slice(&(function_index as u64).to_le_bytes());
    canonical.extend_from_slice(&load.0.to_le_bytes());
    Ok(ValidatedStoredLoadForwarding {
        receipt: StoredLoadForwardingReceipt {
            identity: selected_instructions::StoredLoadForwardingIdentity::from_canonical_bytes(
                &canonical,
            ),
            function_index,
            load,
            source_selected: source.selected_identity(),
            transformed_selected: selected_instruction_plan_identity(&proposed),
            optimization_unit: source.optimization_unit_identity(),
            fuel_schedule: source.fuel_schedule_identity(),
        },
        transformed: Arc::new(proposed),
    })
}
