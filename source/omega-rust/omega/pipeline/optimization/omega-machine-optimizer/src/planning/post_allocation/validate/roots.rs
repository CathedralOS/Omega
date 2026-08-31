use omega_regalloc::{
    ValidatedAllocationLegality, ValidatedLiveRanges, ValidatedPostAllocationOptimizationManifest,
    ValidatedRegisterHomes, ValidatedSelectedAnalysis,
};
use omega_register_model::{
    TargetRegisterEnvironmentIdentity, ValidatedPhysicalRegisterModel,
    ValidatedRegisterConstraintCatalog,
};

use crate::{
    MachineAlternativeChoiceRule, PostAllocationMachineError, PostAllocationMachinePlan,
    ValidatedPreAllocationMachineEffects,
};

use super::structural::validate_structural_allocation;

#[allow(clippy::too_many_arguments)]
pub(super) fn validate_roots<S: ValidatedSelectedAnalysis>(
    selected: &S,
    effects: &ValidatedPreAllocationMachineEffects,
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    homes: &ValidatedRegisterHomes,
    manifest: &ValidatedPostAllocationOptimizationManifest,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    plan: &PostAllocationMachinePlan,
) -> Result<(), PostAllocationMachineError> {
    validate_structural_allocation(selected, effects, ranges, legality, homes)?;
    if effects.receipt().selected() != selected.selected_identity()
        || ranges.receipt().selected() != selected.selected_identity()
        || plan.selected != selected.selected_identity()
    {
        return Err(PostAllocationMachineError::SelectedRootMismatch);
    }
    if plan.effects != effects.receipt().identity() {
        return Err(PostAllocationMachineError::EffectRootMismatch);
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
    if legality.receipt().ranges() != ranges.receipt().identity()
        || plan.ranges != ranges.receipt().identity()
    {
        return Err(PostAllocationMachineError::RangeRootMismatch);
    }
    if plan.legality != legality.receipt().identity() {
        return Err(PostAllocationMachineError::LegalityRootMismatch);
    }
    if homes.receipt().ranges() != ranges.receipt().identity()
        || homes.receipt().legality() != legality.receipt().identity()
        || plan.homes != homes.receipt().identity()
    {
        return Err(PostAllocationMachineError::HomeRootMismatch);
    }
    if effects.plan().register_environment != register_environment
        || legality.receipt().register_environment() != register_environment
        || homes.receipt().register_environment() != register_environment
        || plan.register_environment != register_environment
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
        || plan.post_allocation_manifest != record.identity
    {
        return Err(PostAllocationMachineError::PostAllocationManifestMismatch);
    }
    if effects.plan().target != selected.selected_plan().target
        || plan.target != selected.selected_plan().target
    {
        return Err(PostAllocationMachineError::TargetMismatch);
    }
    if physical.model().architecture != selected.selected_plan().target.architecture
        || plan.physical_register_model != physical.identity()
    {
        return Err(PostAllocationMachineError::PhysicalRegisterModelMismatch);
    }
    if constraints.physical_identity() != physical.identity()
        || constraints.identity() != effects.plan().register_constraints
        || plan.register_constraints != constraints.identity()
    {
        return Err(PostAllocationMachineError::RegisterConstraintCatalogMismatch);
    }
    if plan.register_constraints != effects.plan().register_constraints
        || plan.machine_effect_catalog != effects.plan().machine_effect_catalog
        || plan.choice_rule != MachineAlternativeChoiceRule::UniqueApplicableInCatalogOrderV1
    {
        return Err(PostAllocationMachineError::EffectRootMismatch);
    }
    Ok(())
}
