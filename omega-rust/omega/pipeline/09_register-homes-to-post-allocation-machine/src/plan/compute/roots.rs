//! Identity-root and structural-roster admission before construction.

use register_model::{
    TargetRegisterEnvironmentIdentity, ValidatedPhysicalRegisterModel,
    ValidatedRegisterConstraintCatalog,
};
use selected_instructions_to_register_homes::{
    ValidatedAllocationLegality, ValidatedLiveRanges, ValidatedPostAllocationOptimizationManifest,
    ValidatedRegisterHomes, ValidatedSelectedAnalysis,
};

use crate::PostAllocationMachineError;
use selected_instructions_to_register_homes::ValidatedPreAllocationMachineEffects;

#[allow(clippy::too_many_arguments)]
pub(super) fn validate<S: ValidatedSelectedAnalysis>(
    selected: &S,
    effects: &ValidatedPreAllocationMachineEffects,
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    homes: &ValidatedRegisterHomes,
    manifest: &ValidatedPostAllocationOptimizationManifest,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
) -> Result<(), PostAllocationMachineError> {
    if effects.receipt().selected() != selected.selected_identity()
        || ranges.receipt().selected() != selected.selected_identity()
    {
        return Err(PostAllocationMachineError::SelectedRootMismatch);
    }
    if effects.plan().optimization_unit != selected.optimization_unit_identity()
        || ranges.receipt().optimization_unit() != selected.optimization_unit_identity()
    {
        return Err(PostAllocationMachineError::OptimizationUnitMismatch);
    }
    if effects.plan().fuel_schedule != selected.fuel_schedule_identity()
        || ranges.receipt().fuel_schedule() != selected.fuel_schedule_identity()
    {
        return Err(PostAllocationMachineError::FuelScheduleMismatch);
    }
    if legality.receipt().ranges() != ranges.receipt().identity() {
        return Err(PostAllocationMachineError::RangeRootMismatch);
    }
    if homes.receipt().ranges() != ranges.receipt().identity()
        || homes.receipt().legality() != legality.receipt().identity()
    {
        return Err(PostAllocationMachineError::HomeRootMismatch);
    }
    if effects.plan().register_environment != register_environment
        || legality.receipt().register_environment() != register_environment
        || homes.receipt().register_environment() != register_environment
    {
        return Err(PostAllocationMachineError::RegisterEnvironmentMismatch);
    }
    let record = manifest.record();
    if record.target != selected.selected_plan().target
        || record.selected != selected.selected_identity()
        || record.ranges != ranges.receipt().identity()
        || record.legality != legality.receipt().identity()
        || record.homes != homes.receipt().identity()
        || record.register_environment != register_environment
    {
        return Err(PostAllocationMachineError::PostAllocationManifestMismatch);
    }
    if effects.plan().target != selected.selected_plan().target {
        return Err(PostAllocationMachineError::TargetMismatch);
    }
    if physical.model().architecture != selected.selected_plan().target.architecture {
        return Err(PostAllocationMachineError::PhysicalRegisterModelMismatch);
    }
    if constraints.physical_identity() != physical.identity()
        || constraints.identity() != effects.plan().register_constraints
    {
        return Err(PostAllocationMachineError::RegisterConstraintCatalogMismatch);
    }
    Ok(())
}
