use omega_machine_optimizer::{
    TerminalMachineEffectError, ValidatedTerminalPreAllocationMachineEffects,
    analyze_terminal_pre_allocation_machine_effects,
    validate_terminal_pre_allocation_machine_effects,
};
use omega_terminal_isa_aarch64::{
    Aarch64TerminalMachineEffectCatalogValidationError, aarch64_terminal_machine_effect_catalog,
    validate_aarch64_terminal_machine_effect_catalog,
};
use omega_terminal_isa_x86_64::{
    X86_64TerminalMachineEffectCatalogValidationError,
    validate_x86_64_terminal_machine_effect_catalog, x86_64_terminal_machine_effect_catalog,
};

use crate::{
    OptimizedSelectionCustodyError, StagedOptimizedSelectedInstructions,
    StagedOptimizedSelectionCustodyReceipt, validate_optimized_selection_custody,
};

/// Borrowed, non-authoritative pre-allocation machine-effect sidecar with the
/// exact selected-stage custody receipt it describes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedOptimizedMachineEffects {
    effects: ValidatedTerminalPreAllocationMachineEffects,
    custody: StagedOptimizedMachineEffectCustodyReceipt,
}

impl StagedOptimizedMachineEffects {
    pub const fn effects(&self) -> &ValidatedTerminalPreAllocationMachineEffects {
        &self.effects
    }

    pub const fn custody(&self) -> StagedOptimizedMachineEffectCustodyReceipt {
        self.custody
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StagedOptimizedMachineEffectCustodyReceipt {
    source: StagedOptimizedSelectionCustodyReceipt,
    effects: omega_machine_optimizer::TerminalPreAllocationMachineEffectIdentity,
    catalog: omega_terminal_selected_instructions::TerminalMachineEffectCatalogIdentity,
    instruction_count: usize,
}

impl StagedOptimizedMachineEffectCustodyReceipt {
    pub const fn source(self) -> StagedOptimizedSelectionCustodyReceipt {
        self.source
    }
    pub const fn effects(
        self,
    ) -> omega_machine_optimizer::TerminalPreAllocationMachineEffectIdentity {
        self.effects
    }
    pub const fn catalog(
        self,
    ) -> omega_terminal_selected_instructions::TerminalMachineEffectCatalogIdentity {
        self.catalog
    }
    pub const fn instruction_count(self) -> usize {
        self.instruction_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedMachineEffectPipelineError {
    Upstream(OptimizedSelectionCustodyError),
    X86_64Catalog(X86_64TerminalMachineEffectCatalogValidationError),
    Aarch64Catalog(Aarch64TerminalMachineEffectCatalogValidationError),
    Analysis(TerminalMachineEffectError),
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
        source.selected(),
    )
    .map_err(OptimizedMachineEffectPipelineError::Upstream)?;
    let environment = source.register_environment();
    let catalog = validated_catalog(source)?;
    let effects = analyze_terminal_pre_allocation_machine_effects(
        source.selected(),
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        environment.allocation_constraint_keys(),
        &catalog,
    )
    .map_err(OptimizedMachineEffectPipelineError::Analysis)?;
    let custody = custody_receipt(source_receipt, &effects);
    Ok(StagedOptimizedMachineEffects { effects, custody })
}

pub fn validate_optimized_machine_effect_custody(
    source: &StagedOptimizedSelectedInstructions,
    effects: &ValidatedTerminalPreAllocationMachineEffects,
) -> Result<StagedOptimizedMachineEffectCustodyReceipt, OptimizedMachineEffectPipelineError> {
    let source_receipt = validate_optimized_selection_custody(
        source.optimized_target(),
        source.register_environment(),
        source.selected(),
    )
    .map_err(OptimizedMachineEffectPipelineError::Upstream)?;
    let environment = source.register_environment();
    let catalog = validated_catalog(source)?;
    let replayed = validate_terminal_pre_allocation_machine_effects(
        source.selected(),
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
    Ok(custody_receipt(source_receipt, &replayed))
}

fn validated_catalog(
    source: &StagedOptimizedSelectedInstructions,
) -> Result<
    omega_terminal_selected_instructions::ValidatedTerminalMachineEffectCatalog,
    OptimizedMachineEffectPipelineError,
> {
    let target = source.optimized_target().target();
    let constraints = source.register_environment().constraints();
    match target.architecture {
        omega_target::Architecture::X86_64 => {
            let catalog = x86_64_terminal_machine_effect_catalog(target, constraints)
                .map_err(OptimizedMachineEffectPipelineError::X86_64Catalog)?;
            validate_x86_64_terminal_machine_effect_catalog(target, constraints, catalog)
                .map_err(OptimizedMachineEffectPipelineError::X86_64Catalog)
        }
        omega_target::Architecture::Aarch64 => {
            let catalog = aarch64_terminal_machine_effect_catalog(target, constraints)
                .map_err(OptimizedMachineEffectPipelineError::Aarch64Catalog)?;
            validate_aarch64_terminal_machine_effect_catalog(target, constraints, catalog)
                .map_err(OptimizedMachineEffectPipelineError::Aarch64Catalog)
        }
    }
}

fn custody_receipt(
    source: StagedOptimizedSelectionCustodyReceipt,
    effects: &ValidatedTerminalPreAllocationMachineEffects,
) -> StagedOptimizedMachineEffectCustodyReceipt {
    StagedOptimizedMachineEffectCustodyReceipt {
        source,
        effects: effects.receipt().identity(),
        catalog: effects.receipt().machine_effect_catalog(),
        instruction_count: effects.receipt().instruction_count(),
    }
}
