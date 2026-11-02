//! Optimizer module role: executable entrance. Independent admission of selected-form bytes.
//!
//! Ordinary rows descend into target-owned byte decoders. Symbolic addresses
//! are checked against retained frame geometry before aggregate custody.

use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions::CurrentSelectedProgram;

use physical_instructions::PostAllocationMachinePlan;

use super::OptimizedSelectedFormEncodingError;
use machine_code::SelectedFormEncoding;

mod aggregate;
mod ordinary;
pub(crate) mod row;

pub(super) fn validate(
    selected: CurrentSelectedProgram<'_>,
    machine: &PostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    frame: Option<&machine_code::TargetFrameLayoutPlan>,
    artifact: &SelectedFormEncoding,
) -> Result<(), OptimizedSelectedFormEncodingError> {
    crate::frame_address::validate_frame_root(machine, frame)?;
    if artifact.frame.as_ref() != frame {
        return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
    }
    if machine.selected != selected.identity || artifact.selected != selected.identity {
        return Err(OptimizedSelectedFormEncodingError::SelectedRootMismatch);
    }
    if machine.physical_register_model != physical.identity() {
        return Err(OptimizedSelectedFormEncodingError::PhysicalModelMismatch);
    }
    if artifact.machine != machine.identity {
        return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
    }
    if artifact.post_allocation_machine_optimization.is_some() {
        return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
    }

    ordinary::validate(
        selected,
        machine,
        physical,
        frame,
        artifact.rows(),
        artifact.encoded(),
    )?;
    aggregate::validate(artifact)
}
