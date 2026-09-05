//! Optimizer module role: executable entrance. Independent admission of producer-owned selected-form bytes.
//!
//! Ordinary and structural rows descend separately into target-owned byte
//! decoders. Aggregate custody is checked only after both row families pass.

use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions_to_register_homes::ValidatedSelectedAnalysis;

use crate::{
    StagedOptimizedPostAllocationMachineOptimization, StagedOptimizedPostAllocationMachinePlan,
};

use super::{
    OptimizedSelectedFormEncodingError, StagedOptimizedSelectedFormEncoding,
    custody::validate_optimization_roots,
};

mod aggregate;
mod ordinary;
mod row;
mod structural;

pub(super) fn validate<S: ValidatedSelectedAnalysis>(
    selected: &S,
    staged: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    optimization: Option<&StagedOptimizedPostAllocationMachineOptimization>,
    artifact: &StagedOptimizedSelectedFormEncoding,
) -> Result<(), OptimizedSelectedFormEncodingError> {
    let machine = staged.machine().plan();
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
    let expected_optimization = optimization
        .map(|optimization| validate_optimization_roots(selected, staged, physical, optimization))
        .transpose()?;
    if artifact.post_allocation_machine_optimization != expected_optimization {
        return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
    }

    ordinary::validate(selected, staged, physical, optimization, artifact.rows())?;
    structural::validate(
        selected,
        staged,
        physical,
        artifact.structural_unit_functions(),
    )?;
    aggregate::validate(artifact)
}
