use omega_machine_optimizer::{
    ValidatedPreAllocationMachineEffects, analyze_pre_allocation_machine_effects,
    validate_pre_allocation_machine_effects,
};

use crate::{StagedOptimizedSelectedInstructions, ValidatedTargetRegisterEnvironment};

use super::catalog::validated_catalog;
use super::model::OptimizedMachineEffectPipelineError;

pub(super) fn analyze<S: omega_regalloc::ValidatedSelectedAnalysis>(
    selected: &S,
    selected_stage: &StagedOptimizedSelectedInstructions,
    environment: &ValidatedTargetRegisterEnvironment,
) -> Result<ValidatedPreAllocationMachineEffects, OptimizedMachineEffectPipelineError> {
    let catalog = validated_catalog(selected_stage)?;
    analyze_pre_allocation_machine_effects(
        selected,
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        environment.allocation_constraint_keys(),
        &catalog,
    )
    .map_err(OptimizedMachineEffectPipelineError::Analysis)
}

pub(super) fn revalidate<S: omega_regalloc::ValidatedSelectedAnalysis>(
    selected: &S,
    selected_stage: &StagedOptimizedSelectedInstructions,
    environment: &crate::ValidatedTargetRegisterEnvironment,
    effects: &ValidatedPreAllocationMachineEffects,
) -> Result<ValidatedPreAllocationMachineEffects, OptimizedMachineEffectPipelineError> {
    let catalog = validated_catalog(selected_stage)?;
    let replayed = validate_pre_allocation_machine_effects(
        selected,
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        environment.allocation_constraint_keys(),
        &catalog,
        effects.plan().clone(),
    )
    .map_err(OptimizedMachineEffectPipelineError::Analysis)?;
    if &replayed != effects {
        return Err(OptimizedMachineEffectPipelineError::ReceiptMismatch);
    }
    Ok(replayed)
}
