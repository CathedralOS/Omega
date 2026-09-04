#![forbid(unsafe_code)]

//! Optimizer module role: executable entrance. Active-resident selected-form encoding custody stage.
//!
//! Construction, receipt projection, independent replay, retained state, and
//! corruption fixtures descend into named leaves. This entrance alone admits
//! the constructed pre-layout carrier after complete replay.

mod construction;
mod custody;
mod model;
mod validation;

#[cfg(feature = "test-support")]
mod test_support;

pub use model::*;
pub use validation::validate_optimized_active_resident_rematerialization_selected_form_encoding;

#[cfg(feature = "test-support")]
#[doc(hidden)]
pub use test_support::corrupt_active_resident_selected_form_encoding_byte_for_test;

use omega_allocation_legality_to_active_resident_rematerialization::{
    OptimizedActiveResidentRematerializationError, StagedOptimizedActiveResidentRematerialization,
    StagedOptimizedActiveResidentRematerializationCustodyReceipt,
    validate_optimized_active_resident_rematerialization,
};
use omega_post_allocation_machine_to_selected_form_encoding::{
    OptimizedSelectedFormEncodingError, SelectedFormEncodingIdentity, SelectedFormEncodingState,
    StagedOptimizedSelectedFormEncoding, stage_optimized_layout_independent_selected_form_encoding,
    validate_optimized_layout_independent_selected_form_encoding,
};
use omega_register_homes_to_post_allocation_machine::{
    OptimizedPostAllocationMachinePipelineError,
    StagedOptimizedPostAllocationMachineCustodyReceipt, StagedOptimizedPostAllocationMachinePlan,
    validate_optimized_post_allocation_machine_plan_after_active_resident_rematerialization_custody,
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
