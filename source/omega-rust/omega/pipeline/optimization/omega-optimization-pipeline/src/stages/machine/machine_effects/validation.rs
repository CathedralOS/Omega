use omega_machine_optimizer::ValidatedPreAllocationMachineEffects;

use crate::{
    StagedOptimizedActiveResidentRematerialization, StagedOptimizedFixedViewCopies,
    StagedOptimizedLiteralFolds, StagedOptimizedSelectedInstructions,
    StagedSelectedLoweringOptimizationRun, validate_optimized_active_resident_rematerialization,
    validate_optimized_fixed_view_copy_custody, validate_optimized_literal_fold_custody,
    validate_optimized_selection_custody, validate_selected_lowering_optimization_custody,
};

use super::analysis::revalidate;
use super::custody::custody_receipt;
use super::model::{
    OptimizedMachineEffectPipelineError, StagedOptimizedMachineEffectCustodyReceipt,
    StagedOptimizedMachineEffectSourceCustodyReceipt,
};

pub fn validate_optimized_machine_effect_custody(
    source: &StagedOptimizedSelectedInstructions,
    effects: &ValidatedPreAllocationMachineEffects,
) -> Result<StagedOptimizedMachineEffectCustodyReceipt, OptimizedMachineEffectPipelineError> {
    let source_receipt = validate_optimized_selection_custody(
        source.optimized_target(),
        source.register_environment(),
        source.legalized(),
        source.selected(),
    )
    .map_err(OptimizedMachineEffectPipelineError::Upstream)?;
    let environment = source.register_environment();
    let replayed = revalidate(source.selected(), source, environment, effects)?;
    Ok(custody_receipt(
        StagedOptimizedMachineEffectSourceCustodyReceipt::Selected(source_receipt),
        &replayed,
    ))
}

pub fn validate_optimized_machine_effect_custody_after_fixed_view_copies(
    source: &StagedOptimizedFixedViewCopies,
    effects: &ValidatedPreAllocationMachineEffects,
) -> Result<StagedOptimizedMachineEffectCustodyReceipt, OptimizedMachineEffectPipelineError> {
    let source_receipt = validate_optimized_fixed_view_copy_custody(
        source.source_segment_home_stage(),
        source.copies(),
    )
    .map_err(OptimizedMachineEffectPipelineError::FixedViewCopies)?;
    let selected_stage = source
        .source_legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    let replayed = revalidate(
        source.copies(),
        selected_stage,
        selected_stage.register_environment(),
        effects,
    )?;
    Ok(custody_receipt(
        StagedOptimizedMachineEffectSourceCustodyReceipt::FixedViewCopies(source_receipt),
        &replayed,
    ))
}

pub fn validate_optimized_machine_effect_custody_after_literal_folds(
    source: &StagedOptimizedLiteralFolds,
    effects: &ValidatedPreAllocationMachineEffects,
) -> Result<StagedOptimizedMachineEffectCustodyReceipt, OptimizedMachineEffectPipelineError> {
    let source_receipt = validate_optimized_literal_fold_custody(source)
        .map_err(OptimizedMachineEffectPipelineError::LiteralFolds)?;
    let selected_stage = source
        .source_legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    let replayed = revalidate(
        source.final_step().fold(),
        selected_stage,
        selected_stage.register_environment(),
        effects,
    )?;
    Ok(custody_receipt(
        StagedOptimizedMachineEffectSourceCustodyReceipt::LiteralFolds(source_receipt),
        &replayed,
    ))
}

pub fn validate_optimized_machine_effect_custody_after_selected_lowering(
    source: &StagedSelectedLoweringOptimizationRun,
    effects: &ValidatedPreAllocationMachineEffects,
) -> Result<StagedOptimizedMachineEffectCustodyReceipt, OptimizedMachineEffectPipelineError> {
    let source_receipt = validate_selected_lowering_optimization_custody(source)
        .map_err(OptimizedMachineEffectPipelineError::SelectedLowering)?;
    let selected_stage = source
        .source_legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    let environment = selected_stage.register_environment();
    let replayed = match source.steps().last() {
        Some(step) => revalidate(step.fold(), selected_stage, environment, effects)?,
        None => revalidate(
            selected_stage.selected(),
            selected_stage,
            environment,
            effects,
        )?,
    };
    Ok(custody_receipt(
        StagedOptimizedMachineEffectSourceCustodyReceipt::SelectedLowering(source_receipt),
        &replayed,
    ))
}

pub fn validate_optimized_machine_effect_custody_after_active_resident_rematerialization(
    source: &StagedOptimizedActiveResidentRematerialization,
    effects: &ValidatedPreAllocationMachineEffects,
) -> Result<StagedOptimizedMachineEffectCustodyReceipt, OptimizedMachineEffectPipelineError> {
    let source_receipt = validate_optimized_active_resident_rematerialization(source)
        .map_err(OptimizedMachineEffectPipelineError::ActiveResidentRematerialization)?;
    let selected_stage = source
        .source()
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    let replayed = revalidate(
        source.rematerialization(),
        selected_stage,
        selected_stage.register_environment(),
        effects,
    )?;
    Ok(custody_receipt(
        StagedOptimizedMachineEffectSourceCustodyReceipt::ActiveResidentRematerialization(
            source_receipt,
        ),
        &replayed,
    ))
}
