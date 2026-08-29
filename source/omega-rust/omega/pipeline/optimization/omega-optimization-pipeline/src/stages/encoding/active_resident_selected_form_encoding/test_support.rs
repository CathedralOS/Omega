use crate::SelectedFormEncodingState;

use super::StagedOptimizedActiveResidentRematerializationSelectedFormEncoding;

pub(crate) fn corrupt_active_resident_selected_form_encoding_byte_for_test(
    staged: &mut StagedOptimizedActiveResidentRematerializationSelectedFormEncoding,
) {
    let bytes = staged
        .encoding
        .rows_mut()
        .iter_mut()
        .find_map(|row| match &mut row.state {
            SelectedFormEncodingState::Encoded { bytes, .. } => Some(bytes),
            SelectedFormEncodingState::DeferredControl { .. } => None,
        })
        .expect("active-resident fixture must retain one scalar encoding");
    bytes[0] ^= 1;
}
