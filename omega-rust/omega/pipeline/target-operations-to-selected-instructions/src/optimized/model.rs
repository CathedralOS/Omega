use abstract_operations_to_target_operations::ValidatedOptimizedTargetOperations;
pub use selected_instructions::SelectionCustodyReceipt;

use crate::{
    LegalizationError, SelectedInstructionError, ValidatedLegalizedOperations,
    ValidatedSelectedInstructions,
};
use register_environment::ValidatedTargetRegisterEnvironment;

/// Opt-in selected-instruction staging with complete optimized lowering and
/// target-register custody. This grants no liveness, allocation, emission, or
/// publication authority.
#[derive(Debug)]
pub struct StagedOptimizedSelectedInstructions {
    pub(super) optimized_target: std::sync::Arc<ValidatedOptimizedTargetOperations>,
    pub(super) register_environment: ValidatedTargetRegisterEnvironment,
    pub(super) legalized: ValidatedLegalizedOperations,
    pub(super) selected: ValidatedSelectedInstructions,
    pub(super) custody: SelectionCustodyReceipt,
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

    pub const fn custody(&self) -> SelectionCustodyReceipt {
        self.custody
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
