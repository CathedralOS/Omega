use omega_abstract_operations_to_target_operations::ValidatedOptimizedTargetOperations;
use omega_optimization_core::{
    OptimizationIdentityBundleIdentity, OptimizationUnitIdentity, OptimizationValidatorIdentity,
    OptimizedAbstractPlanProjectionIdentity, PrePhysicalOptimizationManifestIdentity,
};
use omega_selected_instructions::SelectedInstructionPlanIdentity;
use psi_core::{FuelScheduleIdentity, MachineId};
use psi_terminal::TerminalPsiIdentity;

use crate::{
    LegalizationError, SelectedInstructionError, ValidatedLegalizedOperations,
    ValidatedSelectedInstructions,
};
use omega_target_to_register_environment::ValidatedTargetRegisterEnvironment;

/// Opt-in selected-instruction staging with complete optimized lowering and
/// target-register custody. This grants no liveness, allocation, emission, or
/// publication authority.
#[derive(Debug)]
pub struct StagedOptimizedSelectedInstructions {
    pub(super) optimized_target: std::sync::Arc<ValidatedOptimizedTargetOperations>,
    pub(super) register_environment: ValidatedTargetRegisterEnvironment,
    pub(super) legalized: ValidatedLegalizedOperations,
    pub(super) selected: ValidatedSelectedInstructions,
    pub(super) custody: StagedOptimizedSelectionCustodyReceipt,
}

impl StagedOptimizedSelectedInstructions {
    /// Retained upstream proof input, shared without moving it into program data.
    pub fn optimized_target_owner(&self) -> &std::sync::Arc<ValidatedOptimizedTargetOperations> {
        &self.optimized_target
    }

    pub fn optimized_target(&self) -> &ValidatedOptimizedTargetOperations {
        &self.optimized_target
    }

    pub const fn register_environment(&self) -> &ValidatedTargetRegisterEnvironment {
        &self.register_environment
    }

    pub const fn legalized(&self) -> &ValidatedLegalizedOperations {
        &self.legalized
    }

    pub const fn selected(&self) -> &ValidatedSelectedInstructions {
        &self.selected
    }

    pub const fn custody(&self) -> StagedOptimizedSelectionCustodyReceipt {
        self.custody
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StagedOptimizedSelectionCustodyReceipt {
    pub(super) psi: TerminalPsiIdentity,
    pub(super) target: omega_target::NativeTarget,
    pub(super) entry: MachineId,
    pub(super) optimization: OptimizationIdentityBundleIdentity,
    pub(super) projection: OptimizedAbstractPlanProjectionIdentity,
    pub(super) manifest: PrePhysicalOptimizationManifestIdentity,
    pub(super) optimization_unit: OptimizationUnitIdentity,
    pub(super) fuel_schedule: FuelScheduleIdentity,
    pub(super) register_environment: omega_register_model::TargetRegisterEnvironmentIdentity,
    pub(super) legalized: omega_legalized_operations::LegalizedOperationPlanIdentity,
    pub(super) legalization_validator: OptimizationValidatorIdentity,
    pub(super) selected: SelectedInstructionPlanIdentity,
    pub(super) function_count: usize,
}

impl StagedOptimizedSelectionCustodyReceipt {
    pub const fn psi(self) -> TerminalPsiIdentity {
        self.psi
    }

    pub const fn target(self) -> omega_target::NativeTarget {
        self.target
    }

    pub const fn entry(self) -> MachineId {
        self.entry
    }

    pub const fn optimization(self) -> OptimizationIdentityBundleIdentity {
        self.optimization
    }

    pub const fn projection(self) -> OptimizedAbstractPlanProjectionIdentity {
        self.projection
    }

    pub const fn manifest(self) -> PrePhysicalOptimizationManifestIdentity {
        self.manifest
    }

    pub const fn optimization_unit(self) -> OptimizationUnitIdentity {
        self.optimization_unit
    }

    pub const fn fuel_schedule(self) -> FuelScheduleIdentity {
        self.fuel_schedule
    }

    pub const fn selected(self) -> SelectedInstructionPlanIdentity {
        self.selected
    }

    pub const fn legalized(self) -> omega_legalized_operations::LegalizedOperationPlanIdentity {
        self.legalized
    }

    pub const fn legalization_validator(self) -> OptimizationValidatorIdentity {
        self.legalization_validator
    }

    pub const fn register_environment(
        self,
    ) -> omega_register_model::TargetRegisterEnvironmentIdentity {
        self.register_environment
    }

    pub const fn function_count(self) -> usize {
        self.function_count
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizedSelectionCustodyError {
    RootMismatch,
    RegisterEnvironmentTargetMismatch,
    UnitIdentityMismatch,
    FuelScheduleMismatch,
    FunctionRosterMismatch,
    LegalizedPlanRevalidationFailed,
    LegalizedReceiptMismatch,
    SelectedPlanRevalidationFailed,
    SelectedReceiptMismatch,
}

#[derive(Debug)]
pub enum OptimizedSelectionPipelineError {
    Legalization(LegalizationError),
    Selection(SelectedInstructionError),
    Custody(OptimizedSelectionCustodyError),
}

impl std::fmt::Display for OptimizedSelectionPipelineError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "optimized instruction selection failed: {self:?}"
        )
    }
}

impl std::error::Error for OptimizedSelectionPipelineError {}
