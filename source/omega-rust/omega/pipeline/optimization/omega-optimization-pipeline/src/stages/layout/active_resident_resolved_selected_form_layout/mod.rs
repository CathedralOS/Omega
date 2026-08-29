//! Active-resident resolved selected-form layout stage.
//!
//! Layout construction, custody aggregation, independent replay, retained
//! state, and corruption fixtures descend into named leaves. This entrance
//! alone admits the resolved-layout carrier after complete replay.

mod construction;
mod custody;
mod model;
mod validation;

#[cfg(test)]
mod test_support;

pub use model::*;
pub use validation::validate_optimized_active_resident_rematerialization_resolved_selected_form_layout;

#[cfg(test)]
pub(crate) use test_support::{
    corrupt_active_resident_resolved_layout_byte_for_test,
    corrupt_active_resident_resolved_layout_pre_layout_byte_for_test,
    corrupt_active_resident_resolved_layout_receipt_for_test,
};

use crate::StagedOptimizedActiveResidentRematerializationSelectedFormEncoding;

pub fn stage_optimized_active_resident_rematerialization_resolved_selected_form_layout(
    pre_layout: StagedOptimizedActiveResidentRematerializationSelectedFormEncoding,
) -> Result<
    StagedOptimizedActiveResidentRematerializationResolvedSelectedFormLayout,
    OptimizedActiveResidentRematerializationResolvedSelectedFormLayoutError,
> {
    let (pre_layout_custody, physical, layout) =
        construction::construct_active_resident_resolved_selected_form_layout(&pre_layout)?;
    let custody = custody::project_active_resident_resolved_layout_custody(
        pre_layout_custody,
        physical,
        &layout,
    );
    let staged = StagedOptimizedActiveResidentRematerializationResolvedSelectedFormLayout {
        pre_layout,
        layout,
        custody,
    };
    validate_optimized_active_resident_rematerialization_resolved_selected_form_layout(&staged)?;
    Ok(staged)
}
