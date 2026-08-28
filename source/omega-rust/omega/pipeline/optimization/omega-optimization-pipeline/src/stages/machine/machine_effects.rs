use omega_isa_aarch64::{
    Aarch64MachineEffectCatalogValidationError, aarch64_machine_effect_catalog,
    validate_aarch64_machine_effect_catalog,
};
use omega_isa_x86_64::{
    X86_64MachineEffectCatalogValidationError, validate_x86_64_machine_effect_catalog,
    x86_64_machine_effect_catalog,
};
use omega_machine_optimizer::{
    MachineEffectError, ValidatedPreAllocationMachineEffects,
    analyze_pre_allocation_machine_effects, validate_pre_allocation_machine_effects,
};

use crate::{
    OptimizedActiveResidentRematerializationError, OptimizedFixedViewCopyCustodyError,
    OptimizedLiteralFoldCustodyError, OptimizedSelectionCustodyError,
    StagedOptimizedActiveResidentRematerialization,
    StagedOptimizedActiveResidentRematerializationCustodyReceipt, StagedOptimizedFixedViewCopies,
    StagedOptimizedFixedViewCopyCustodyReceipt, StagedOptimizedLiteralFoldCustodyReceipt,
    StagedOptimizedLiteralFolds, StagedOptimizedSelectedInstructions,
    StagedOptimizedSelectionCustodyReceipt, StagedSelectedLoweringOptimizationCustodyReceipt,
    StagedSelectedLoweringOptimizationRun, validate_optimized_active_resident_rematerialization,
    validate_optimized_fixed_view_copy_custody, validate_optimized_literal_fold_custody,
    validate_optimized_selection_custody, validate_selected_lowering_optimization_custody,
};

/// Borrowed, non-authoritative pre-allocation machine-effect sidecar with the
/// exact selected-stage custody receipt it describes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedOptimizedMachineEffects {
    effects: ValidatedPreAllocationMachineEffects,
    custody: StagedOptimizedMachineEffectCustodyReceipt,
}

impl StagedOptimizedMachineEffects {
    pub const fn effects(&self) -> &ValidatedPreAllocationMachineEffects {
        &self.effects
    }

    pub const fn custody(&self) -> &StagedOptimizedMachineEffectCustodyReceipt {
        &self.custody
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedOptimizedMachineEffectCustodyReceipt {
    source: StagedOptimizedMachineEffectSourceCustodyReceipt,
    effects: omega_machine_optimizer::PreAllocationMachineEffectIdentity,
    catalog: omega_selected_instructions::MachineEffectCatalogIdentity,
    instruction_count: usize,
}

impl StagedOptimizedMachineEffectCustodyReceipt {
    pub const fn source(&self) -> &StagedOptimizedMachineEffectSourceCustodyReceipt {
        &self.source
    }
    pub const fn effects(&self) -> omega_machine_optimizer::PreAllocationMachineEffectIdentity {
        self.effects
    }
    pub const fn catalog(&self) -> omega_selected_instructions::MachineEffectCatalogIdentity {
        self.catalog
    }
    pub const fn instruction_count(&self) -> usize {
        self.instruction_count
    }
}

/// The independently revalidated custody carrier whose selected CFG the
/// effect sidecar describes. Transformations cannot be passed off as their
/// pre-transformation selected source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StagedOptimizedMachineEffectSourceCustodyReceipt {
    Selected(StagedOptimizedSelectionCustodyReceipt),
    FixedViewCopies(StagedOptimizedFixedViewCopyCustodyReceipt),
    LiteralFolds(StagedOptimizedLiteralFoldCustodyReceipt),
    SelectedLowering(StagedSelectedLoweringOptimizationCustodyReceipt),
    ActiveResidentRematerialization(StagedOptimizedActiveResidentRematerializationCustodyReceipt),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedMachineEffectPipelineError {
    Upstream(OptimizedSelectionCustodyError),
    FixedViewCopies(OptimizedFixedViewCopyCustodyError),
    LiteralFolds(OptimizedLiteralFoldCustodyError),
    SelectedLowering(OptimizedLiteralFoldCustodyError),
    ActiveResidentRematerialization(OptimizedActiveResidentRematerializationError),
    X86_64Catalog(X86_64MachineEffectCatalogValidationError),
    Aarch64Catalog(Aarch64MachineEffectCatalogValidationError),
    Analysis(MachineEffectError),
    ReceiptMismatch,
}

impl std::fmt::Display for OptimizedMachineEffectPipelineError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "optimized machine-effect staging failed: {self:?}"
        )
    }
}

impl std::error::Error for OptimizedMachineEffectPipelineError {}

pub fn stage_optimized_machine_effects(
    source: &StagedOptimizedSelectedInstructions,
) -> Result<StagedOptimizedMachineEffects, OptimizedMachineEffectPipelineError> {
    let source_receipt = validate_optimized_selection_custody(
        source.optimized_target(),
        source.register_environment(),
        source.legalized(),
        source.selected(),
    )
    .map_err(OptimizedMachineEffectPipelineError::Upstream)?;
    let environment = source.register_environment();
    let effects = analyze(source.selected(), source, environment)?;
    let custody = custody_receipt(
        StagedOptimizedMachineEffectSourceCustodyReceipt::Selected(source_receipt),
        &effects,
    );
    Ok(StagedOptimizedMachineEffects { effects, custody })
}

pub fn stage_optimized_machine_effects_after_fixed_view_copies(
    source: &StagedOptimizedFixedViewCopies,
) -> Result<StagedOptimizedMachineEffects, OptimizedMachineEffectPipelineError> {
    let source_receipt =
        validate_optimized_fixed_view_copy_custody(source.source_legality_stage(), source.copies())
            .map_err(OptimizedMachineEffectPipelineError::FixedViewCopies)?;
    let selected_stage = source
        .source_legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    let effects = analyze(
        source.copies(),
        selected_stage,
        selected_stage.register_environment(),
    )?;
    let custody = custody_receipt(
        StagedOptimizedMachineEffectSourceCustodyReceipt::FixedViewCopies(source_receipt),
        &effects,
    );
    Ok(StagedOptimizedMachineEffects { effects, custody })
}

pub fn stage_optimized_machine_effects_after_literal_folds(
    source: &StagedOptimizedLiteralFolds,
) -> Result<StagedOptimizedMachineEffects, OptimizedMachineEffectPipelineError> {
    let source_receipt = validate_optimized_literal_fold_custody(source)
        .map_err(OptimizedMachineEffectPipelineError::LiteralFolds)?;
    let selected_stage = source
        .source_legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    let effects = analyze(
        source.final_step().fold(),
        selected_stage,
        selected_stage.register_environment(),
    )?;
    let custody = custody_receipt(
        StagedOptimizedMachineEffectSourceCustodyReceipt::LiteralFolds(source_receipt),
        &effects,
    );
    Ok(StagedOptimizedMachineEffects { effects, custody })
}

pub fn stage_optimized_machine_effects_after_selected_lowering(
    source: &StagedSelectedLoweringOptimizationRun,
) -> Result<StagedOptimizedMachineEffects, OptimizedMachineEffectPipelineError> {
    let source_receipt = validate_selected_lowering_optimization_custody(source)
        .map_err(OptimizedMachineEffectPipelineError::SelectedLowering)?;
    let selected_stage = source
        .source_legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    let environment = selected_stage.register_environment();
    let effects = match source.steps().last() {
        Some(step) => analyze(step.fold(), selected_stage, environment)?,
        None => analyze(selected_stage.selected(), selected_stage, environment)?,
    };
    let custody = custody_receipt(
        StagedOptimizedMachineEffectSourceCustodyReceipt::SelectedLowering(source_receipt),
        &effects,
    );
    Ok(StagedOptimizedMachineEffects { effects, custody })
}

pub fn stage_optimized_machine_effects_after_active_resident_rematerialization(
    source: &StagedOptimizedActiveResidentRematerialization,
) -> Result<StagedOptimizedMachineEffects, OptimizedMachineEffectPipelineError> {
    let source_receipt = validate_optimized_active_resident_rematerialization(source)
        .map_err(OptimizedMachineEffectPipelineError::ActiveResidentRematerialization)?;
    let selected_stage = source
        .source()
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    let effects = analyze(
        source.rematerialization(),
        selected_stage,
        selected_stage.register_environment(),
    )?;
    let custody = custody_receipt(
        StagedOptimizedMachineEffectSourceCustodyReceipt::ActiveResidentRematerialization(
            source_receipt,
        ),
        &effects,
    );
    Ok(StagedOptimizedMachineEffects { effects, custody })
}

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
    let source_receipt =
        validate_optimized_fixed_view_copy_custody(source.source_legality_stage(), source.copies())
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

fn validated_catalog(
    source: &StagedOptimizedSelectedInstructions,
) -> Result<
    omega_selected_instructions::ValidatedMachineEffectCatalog,
    OptimizedMachineEffectPipelineError,
> {
    let target = source.optimized_target().target();
    let constraints = source.register_environment().constraints();
    match target.architecture {
        omega_target::Architecture::X86_64 => {
            let catalog = x86_64_machine_effect_catalog(target, constraints)
                .map_err(OptimizedMachineEffectPipelineError::X86_64Catalog)?;
            validate_x86_64_machine_effect_catalog(target, constraints, catalog)
                .map_err(OptimizedMachineEffectPipelineError::X86_64Catalog)
        }
        omega_target::Architecture::Aarch64 => {
            let catalog = aarch64_machine_effect_catalog(target, constraints)
                .map_err(OptimizedMachineEffectPipelineError::Aarch64Catalog)?;
            validate_aarch64_machine_effect_catalog(target, constraints, catalog)
                .map_err(OptimizedMachineEffectPipelineError::Aarch64Catalog)
        }
    }
}

fn custody_receipt(
    source: StagedOptimizedMachineEffectSourceCustodyReceipt,
    effects: &ValidatedPreAllocationMachineEffects,
) -> StagedOptimizedMachineEffectCustodyReceipt {
    StagedOptimizedMachineEffectCustodyReceipt {
        source,
        effects: effects.receipt().identity(),
        catalog: effects.receipt().machine_effect_catalog(),
        instruction_count: effects.receipt().instruction_count(),
    }
}

fn analyze<S: omega_regalloc::ValidatedSelectedAnalysis>(
    selected: &S,
    selected_stage: &StagedOptimizedSelectedInstructions,
    environment: &crate::ValidatedTargetRegisterEnvironment,
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

fn revalidate<S: omega_regalloc::ValidatedSelectedAnalysis>(
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
