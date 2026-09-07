#![forbid(unsafe_code)]

//! Optimizer module role: executable entrance. Layout-independent selected-form encoding, replay, and optimization custody.
//!
//! This stage serializes selected instructions before any address-dependent
//! layout and retains exact selected and physical roots.

use machine_code::{
    DeferredControlEncodingReason, SelectedFormDecodedFootprint, SelectedFormEncoding,
    SelectedFormEncodingCounts, SelectedFormEncodingIdentity, SelectedFormEncodingRow,
    SelectedFormEncodingState, SelectedFormInternalMachineFixup,
    SelectedFormInternalMachineFixupKind, SelectedFormInternalMachineFixupState,
    SelectedFormMachineDisposition,
};
use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions_to_register_homes::ValidatedSelectedAnalysis;

use register_homes_to_post_allocation_machine::StagedOptimizedPostAllocationMachinePlan;

mod compute;
mod error;
mod frame_address;
mod model;
mod row_encoding;
mod validation;

pub use error::*;
pub use model::*;

/// Encode the current physical instructions and independently admit their bytes.
pub fn stage_optimized_layout_independent_selected_form_encoding<S: ValidatedSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    frame: Option<&machine_code::TargetFrameLayoutPlan>,
) -> Result<StagedOptimizedSelectedFormEncoding, OptimizedSelectedFormEncodingError> {
    let artifact = compute::compute(selected, machine, physical, frame)?;
    validation::validate(selected, machine, physical, frame, &artifact)?;
    Ok(StagedOptimizedSelectedFormEncoding {
        program: std::sync::Arc::new(artifact),
    })
}

/// Replay the canonical selected-form encoding join against all retained
/// selected, physical-machine, and exact frame roots.
pub fn validate_optimized_layout_independent_selected_form_encoding<
    S: ValidatedSelectedAnalysis,
>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    frame: Option<&machine_code::TargetFrameLayoutPlan>,
    artifact: &StagedOptimizedSelectedFormEncoding,
) -> Result<(), OptimizedSelectedFormEncodingError> {
    validation::validate(selected, machine, physical, frame, artifact.program())
}
