//! Identity-root and structural-roster admission before construction.

use crate::PostAllocationMachineError;
use register_homes::AllocatedProgramFacts;

use super::PlanSources;

pub(super) fn validate(sources: &PlanSources<'_>) -> Result<(), PostAllocationMachineError> {
    let PlanSources {
        allocation,
        register_environment,
        physical,
        constraints,
    } = *sources;
    let AllocatedProgramFacts {
        selected,
        effects,
        ranges,
        ranges_identity,
        legality,
        legality_identity,
        homes,
        homes_identity,
        manifest,
    } = allocation;
    if effects.selected != selected.identity || ranges.selected != selected.identity {
        return Err(PostAllocationMachineError::SelectedRootMismatch);
    }
    if effects.optimization_unit != selected.optimization_unit
        || ranges.optimization_unit != selected.optimization_unit
    {
        return Err(PostAllocationMachineError::OptimizationUnitMismatch);
    }
    if effects.work_schedule != selected.work_schedule
        || ranges.work_schedule != selected.work_schedule
    {
        return Err(PostAllocationMachineError::WorkScheduleMismatch);
    }
    if legality.ranges != ranges_identity {
        return Err(PostAllocationMachineError::RangeRootMismatch);
    }
    if homes.ranges != ranges_identity || homes.legality != legality_identity {
        return Err(PostAllocationMachineError::HomeRootMismatch);
    }
    if effects.register_environment != register_environment
        || legality.register_environment != register_environment
        || homes.register_environment != register_environment
    {
        return Err(PostAllocationMachineError::RegisterEnvironmentMismatch);
    }
    if manifest.target != selected.plan.target
        || manifest.selected != selected.identity
        || manifest.ranges != ranges_identity
        || manifest.legality != legality_identity
        || manifest.homes != homes_identity
        || manifest.register_environment != register_environment
    {
        return Err(PostAllocationMachineError::PostAllocationManifestMismatch);
    }
    if effects.target != selected.plan.target {
        return Err(PostAllocationMachineError::TargetMismatch);
    }
    if physical.model().architecture != selected.plan.target.architecture {
        return Err(PostAllocationMachineError::PhysicalRegisterModelMismatch);
    }
    if constraints.physical_identity() != physical.identity()
        || constraints.identity() != effects.register_constraints
    {
        return Err(PostAllocationMachineError::RegisterConstraintCatalogMismatch);
    }
    Ok(())
}
