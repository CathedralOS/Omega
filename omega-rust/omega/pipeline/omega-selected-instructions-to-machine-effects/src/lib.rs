#![forbid(unsafe_code)]

//! Optimizer module role: executable entrance. Pre-allocation machine-effect staging.
//!
//! Each supported selected-source lineage has one route adapter. This entrance
//! grants a machine-effect sidecar custody only after the analyzed plan
//! independently replays against the exact target effect catalog.

mod analysis;
mod catalog;
mod construction;
mod custody;
mod model;
mod validation;

pub use model::*;
pub use validation::*;

use omega_allocation_legality_to_active_resident_rematerialization::StagedOptimizedActiveResidentRematerialization;
use omega_allocation_legality_to_fixed_view_copies::StagedOptimizedFixedViewCopies;
use omega_allocation_legality_to_literal_folds::{
    StagedOptimizedLiteralFolds, StagedSelectedLoweringOptimizationRun,
};
use omega_target_operations_to_selected_instructions::StagedOptimizedSelectedInstructions;

fn admit_machine_effects(
    staged: StagedOptimizedMachineEffects,
    replayed: StagedOptimizedMachineEffectCustodyReceipt,
) -> Result<StagedOptimizedMachineEffects, OptimizedMachineEffectPipelineError> {
    if staged.custody() != &replayed {
        return Err(OptimizedMachineEffectPipelineError::ReceiptMismatch);
    }
    Ok(staged)
}

pub fn stage_optimized_machine_effects(
    source: &StagedOptimizedSelectedInstructions,
) -> Result<StagedOptimizedMachineEffects, OptimizedMachineEffectPipelineError> {
    let staged = construction::construct_optimized_machine_effects(source)?;
    let replayed = validate_optimized_machine_effect_custody(source, staged.effects())?;
    admit_machine_effects(staged, replayed)
}

pub fn stage_optimized_machine_effects_after_fixed_view_copies(
    source: &StagedOptimizedFixedViewCopies,
) -> Result<StagedOptimizedMachineEffects, OptimizedMachineEffectPipelineError> {
    let staged = construction::construct_optimized_machine_effects_after_fixed_view_copies(source)?;
    let replayed = validate_optimized_machine_effect_custody_after_fixed_view_copies(
        source,
        staged.effects(),
    )?;
    admit_machine_effects(staged, replayed)
}

pub fn stage_optimized_machine_effects_after_literal_folds(
    source: &StagedOptimizedLiteralFolds,
) -> Result<StagedOptimizedMachineEffects, OptimizedMachineEffectPipelineError> {
    let staged = construction::construct_optimized_machine_effects_after_literal_folds(source)?;
    let replayed =
        validate_optimized_machine_effect_custody_after_literal_folds(source, staged.effects())?;
    admit_machine_effects(staged, replayed)
}

pub fn stage_optimized_machine_effects_after_selected_lowering(
    source: &StagedSelectedLoweringOptimizationRun,
) -> Result<StagedOptimizedMachineEffects, OptimizedMachineEffectPipelineError> {
    let staged = construction::construct_optimized_machine_effects_after_selected_lowering(source)?;
    let replayed = validate_optimized_machine_effect_custody_after_selected_lowering(
        source,
        staged.effects(),
    )?;
    admit_machine_effects(staged, replayed)
}

pub fn stage_optimized_machine_effects_after_active_resident_rematerialization(
    source: &StagedOptimizedActiveResidentRematerialization,
) -> Result<StagedOptimizedMachineEffects, OptimizedMachineEffectPipelineError> {
    let staged =
        construction::construct_optimized_machine_effects_after_active_resident_rematerialization(
            source,
        )?;
    let replayed =
        validate_optimized_machine_effect_custody_after_active_resident_rematerialization(
            source,
            staged.effects(),
        )?;
    admit_machine_effects(staged, replayed)
}
