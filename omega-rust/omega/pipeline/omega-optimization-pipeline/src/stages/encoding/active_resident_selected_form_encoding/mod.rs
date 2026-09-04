//! Optimizer module role: executable entrance. Active-resident selected-form encoding custody stage.
//!
//! Construction, receipt projection, independent replay, retained state, and
//! corruption fixtures descend into named leaves. This entrance alone admits
//! the constructed pre-layout carrier after complete replay.

mod construction;
mod custody;
mod model;
mod validation;

#[cfg(test)]
mod test_support;

pub use model::*;
pub use validation::validate_optimized_active_resident_rematerialization_selected_form_encoding;

#[cfg(test)]
pub(crate) use test_support::corrupt_active_resident_selected_form_encoding_byte_for_test;

use crate::{
    StagedOptimizedActiveResidentRematerialization, StagedOptimizedPostAllocationMachinePlan,
};

pub fn stage_optimized_active_resident_rematerialization_selected_form_encoding(
    source: StagedOptimizedActiveResidentRematerialization,
    machine: StagedOptimizedPostAllocationMachinePlan,
) -> Result<
    StagedOptimizedActiveResidentRematerializationSelectedFormEncoding,
    OptimizedActiveResidentRematerializationSelectedFormEncodingError,
> {
    let (rematerialization, machine_custody, encoding) =
        construction::construct_active_resident_selected_form_encoding(&source, &machine)?;
    let custody = custody::project_active_resident_selected_form_encoding_custody(
        rematerialization,
        machine_custody,
        &encoding,
    );
    let staged = StagedOptimizedActiveResidentRematerializationSelectedFormEncoding {
        source,
        machine,
        encoding,
        custody,
    };
    validate_optimized_active_resident_rematerialization_selected_form_encoding(&staged)?;
    Ok(staged)
}
