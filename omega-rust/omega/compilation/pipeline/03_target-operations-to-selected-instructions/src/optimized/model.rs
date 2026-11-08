use optimization_core::{
    OptimizationIdentityBundleIdentity, OptimizationSelections, OptimizationUnitIdentity,
    OptimizationValidatorIdentity, OptimizationWorkBudget, OptimizedAbstractPlanProjectionIdentity,
    PrePhysicalOptimizationManifestIdentity,
};
use selected_instructions::SelectedInstructionPlanIdentity;
use semantic_vocabulary::{MachineId, WorkScheduleIdentity};
use terminal_psi::TerminalPsiIdentity;

use crate::{LegalizationError, SelectedInstructionError, ValidatedSelectedInstructions};
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::StagedOptimizedCustodyCore;

/// Opt-in selected-instruction staging with complete optimized lowering and
/// target-register custody. This grants no liveness, allocation, emission, or
/// publication authority.
#[derive(Debug)]
pub struct StagedOptimizedSelectedInstructions {
    pub(super) register_environment: ValidatedTargetRegisterEnvironment,
    pub(super) selections: OptimizationSelections,
    pub(super) budget_per_pass: OptimizationWorkBudget,
    pub(super) selected: ValidatedSelectedInstructions,
    pub(super) custody: StagedOptimizedSelectionCustodyReceipt,
}

impl StagedOptimizedSelectedInstructions {
    pub const fn register_environment(&self) -> &ValidatedTargetRegisterEnvironment {
        &self.register_environment
    }

    pub const fn selected(&self) -> &ValidatedSelectedInstructions {
        &self.selected
    }

    /// The selected program as the stages after selection carry it.
    pub fn selected_program(&self) -> selected_instructions::SelectedProgram {
        let receipt = self.selected.receipt();
        selected_instructions::SelectedProgram {
            plan: self.selected.shared_plan(),
            identity: receipt.identity(),
            optimization_unit: receipt.optimization_unit(),
            work_schedule: receipt.work_schedule(),
        }
    }

    /// The governing optimizer selections this stage was given, so later
    /// stages read them from selection's output.
    pub const fn selections(&self) -> &OptimizationSelections {
        &self.selections
    }

    /// The per-pass work budget admitted beside the same evidence.
    pub const fn budget_per_pass(&self) -> OptimizationWorkBudget {
        self.budget_per_pass
    }

    pub const fn custody(&self) -> StagedOptimizedSelectionCustodyReceipt {
        self.custody
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StagedOptimizedSelectionCustodyReceipt {
    pub(super) core: StagedOptimizedCustodyCore,
    pub(super) legalized: legalized_operations::LegalizedOperationPlanIdentity,
    pub(super) legalization_validator: OptimizationValidatorIdentity,
    pub(super) selected: SelectedInstructionPlanIdentity,
    pub(super) function_count: usize,
}

impl StagedOptimizedSelectionCustodyReceipt {
    selected_instructions::staging_custody_core_accessors!();

    pub const fn selected(self) -> SelectedInstructionPlanIdentity {
        self.selected
    }

    pub const fn legalized(self) -> legalized_operations::LegalizedOperationPlanIdentity {
        self.legalized
    }

    pub const fn legalization_validator(self) -> OptimizationValidatorIdentity {
        self.legalization_validator
    }

    pub const fn function_count(self) -> usize {
        self.function_count
    }
}

#[derive(Debug)]
pub enum OptimizedSelectionPipelineError {
    Legalization(LegalizationError),
    Selection(SelectedInstructionError),
}

failed_display!(
    OptimizedSelectionPipelineError,
    "optimized instruction selection failed"
);

impl std::error::Error for OptimizedSelectionPipelineError {}
