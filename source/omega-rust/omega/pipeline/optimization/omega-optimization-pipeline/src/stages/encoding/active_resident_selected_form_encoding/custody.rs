use crate::{
    SelectedFormEncodingState, StagedOptimizedActiveResidentRematerializationCustodyReceipt,
    StagedOptimizedPostAllocationMachineCustodyReceipt, StagedOptimizedSelectedFormEncoding,
};

use super::StagedOptimizedActiveResidentRematerializationSelectedFormEncodingCustodyReceipt;

pub(super) fn project_active_resident_selected_form_encoding_custody(
    rematerialization: StagedOptimizedActiveResidentRematerializationCustodyReceipt,
    machine: StagedOptimizedPostAllocationMachineCustodyReceipt,
    encoding: &StagedOptimizedSelectedFormEncoding,
) -> StagedOptimizedActiveResidentRematerializationSelectedFormEncodingCustodyReceipt {
    let encoded_count = encoding
        .rows()
        .iter()
        .filter(|row| matches!(row.state, SelectedFormEncodingState::Encoded { .. }))
        .count();
    let deferred_count = encoding
        .rows()
        .iter()
        .filter(|row| matches!(row.state, SelectedFormEncodingState::DeferredControl { .. }))
        .count();
    StagedOptimizedActiveResidentRematerializationSelectedFormEncodingCustodyReceipt {
        rematerialization,
        machine,
        transformed_selected: encoding.selected(),
        encoding: encoding.identity(),
        row_count: encoding.rows().len(),
        encoded_count,
        deferred_count,
    }
}
