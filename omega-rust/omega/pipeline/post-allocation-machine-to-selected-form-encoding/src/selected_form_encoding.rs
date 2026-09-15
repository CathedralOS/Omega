//! Optimizer module role: executable entrance. Layout-independent selected-form encoding, replay, and optimization custody.
//!
//! Encode the current physical instructions, then independently admit the
//! bytes; replay validates a retained artifact against the same roots.

use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions_to_register_homes::ValidatedSelectedAnalysis;

use register_homes_to_post_allocation_machine::StagedOptimizedPostAllocationMachinePlan;

mod compute;
mod error;
mod model;

pub use error::*;
pub use model::*;

use crate::validation;

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
