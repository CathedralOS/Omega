//! Optimizer module role: executable entrance. Independent admission of selected-form bytes.
//!
//! Ordinary rows descend into target-owned byte decoders. Symbolic addresses
//! are checked against retained frame geometry before aggregate custody.

use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions_to_register_homes::ValidatedSelectedAnalysis;

use crate::StagedOptimizedPostAllocationMachinePlan;

use super::{OptimizedSelectedFormEncodingError, SelectedFormEncoding};

mod aggregate;
mod ordinary;
pub(crate) mod row;

pub(super) fn validate<S: ValidatedSelectedAnalysis>(
    selected: &S,
    staged: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    frame: Option<&machine_code::TargetFrameLayoutPlan>,
    artifact: &SelectedFormEncoding,
) -> Result<(), OptimizedSelectedFormEncodingError> {
    let machine = staged.machine().plan();
    crate::frame_address::validate_frame_root(machine, frame)?;
    if artifact.frame.as_ref() != frame {
        return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
    }
    if machine.selected != selected.selected_identity()
        || artifact.selected != selected.selected_identity()
    {
        return Err(OptimizedSelectedFormEncodingError::SelectedRootMismatch);
    }
    if machine.physical_register_model != physical.identity() {
        return Err(OptimizedSelectedFormEncodingError::PhysicalModelMismatch);
    }
    if artifact.machine != staged.machine().receipt().identity() {
        return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
    }
    if artifact.post_allocation_machine_optimization.is_some() {
        return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
    }

    ordinary::validate(selected, staged, physical, frame, artifact.rows())?;
    aggregate::validate(artifact)
}
