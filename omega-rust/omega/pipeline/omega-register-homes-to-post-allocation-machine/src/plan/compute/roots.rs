//! Identity-root and structural-roster admission before construction.

use omega_register_model::{
    TargetRegisterEnvironmentIdentity, ValidatedPhysicalRegisterModel,
    ValidatedRegisterConstraintCatalog,
};
use omega_selected_instructions_to_register_homes::{
    ValidatedAllocationLegality, ValidatedLiveRanges, ValidatedPostAllocationOptimizationManifest,
    ValidatedRegisterHomes, ValidatedSelectedAnalysis,
};

use crate::PostAllocationMachineError;
use omega_selected_instructions_to_machine_effects::ValidatedPreAllocationMachineEffects;

use super::structural::{unique_effect, unique_home};

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
    validate_structural_allocation(selected, effects, ranges, legality, homes)?;
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

fn validate_structural_allocation<S: ValidatedSelectedAnalysis>(
    selected: &S,
    effects: &ValidatedPreAllocationMachineEffects,
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    homes: &ValidatedRegisterHomes,
) -> Result<(), PostAllocationMachineError> {
    let source = &selected.selected_plan().structural_unit_functions;
    if effects.plan().structural_unit_functions.len() != source.len()
        || ranges.plan().structural_unit_functions.len() != source.len()
        || legality.plan().structural_unit_functions.len() != source.len()
        || homes.plan().structural_unit_functions.len() != source.len()
    {
        let machine = source
            .first()
            .map(|function| function.machine)
            .unwrap_or(selected.selected_plan().entry);
        return Err(PostAllocationMachineError::StructuralAllocationMismatch { machine });
    }
    for function in source {
        unique_effect(effects, function.machine)?;
        let range_matches = ranges
            .plan()
            .structural_unit_functions
            .iter()
            .filter(|candidate| candidate.machine == function.machine)
            .collect::<Vec<_>>();
        let legality_matches = legality
            .plan()
            .structural_unit_functions
            .iter()
            .filter(|candidate| candidate.machine == function.machine)
            .collect::<Vec<_>>();
        let home = unique_home(homes, function.machine)?;
        let ([range], [legality]) = (range_matches.as_slice(), legality_matches.as_slice()) else {
            return Err(PostAllocationMachineError::StructuralAllocationMismatch {
                machine: function.machine,
            });
        };
        if range.block_domains.len() != 1
            || range.block_domains[0].block != function.entry_block
            || !range.virtual_registers.is_empty()
            || !range.tied_pairs.is_empty()
            || !range.early_clobbers.is_empty()
            || !range.interference.is_empty()
            || !legality.virtual_registers.is_empty()
            || !home.assignments.is_empty()
        {
            return Err(PostAllocationMachineError::StructuralAllocationMismatch {
                machine: function.machine,
            });
        }
    }
    Ok(())
}
