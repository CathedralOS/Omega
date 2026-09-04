use omega_isa_aarch64::Aarch64MachineEffectCatalogValidationError;
use omega_isa_x86_64::X86_64MachineEffectCatalogValidationError;
use omega_machine_optimizer::{MachineEffectError, ValidatedPreAllocationMachineEffects};

use omega_allocation_legality_to_active_resident_rematerialization::{
    OptimizedActiveResidentRematerializationError,
    StagedOptimizedActiveResidentRematerializationCustodyReceipt,
};
use omega_allocation_legality_to_fixed_view_copies::{
    OptimizedFixedViewCopyCustodyError, StagedOptimizedFixedViewCopyCustodyReceipt,
};
use omega_allocation_legality_to_literal_folds::{
    OptimizedLiteralFoldCustodyError, StagedOptimizedLiteralFoldCustodyReceipt,
    StagedSelectedLoweringOptimizationCustodyReceipt,
};
use omega_target_operations_to_selected_instructions::{
    OptimizedSelectionCustodyError, StagedOptimizedSelectionCustodyReceipt,
};

/// Borrowed, non-authoritative pre-allocation machine-effect sidecar with the
/// exact selected-stage custody receipt it describes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedOptimizedMachineEffects {
    pub(super) effects: ValidatedPreAllocationMachineEffects,
    pub(super) custody: StagedOptimizedMachineEffectCustodyReceipt,
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
    pub(super) source: StagedOptimizedMachineEffectSourceCustodyReceipt,
    pub(super) effects: omega_machine_optimizer::PreAllocationMachineEffectIdentity,
    pub(super) catalog: omega_selected_instructions::MachineEffectCatalogIdentity,
    pub(super) instruction_count: usize,
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
