//! The selected instructions, register environment and receipt forwarded to stage 06.
use crate::input::BoundExecutionPlan;
use optimization_core::{
    OptimizationIdentityBundleIdentity, OptimizationSelections, OptimizationUnitIdentity,
    OptimizationValidatorIdentity, OptimizationWorkBudget, OptimizedAbstractPlanProjectionIdentity,
    PrePhysicalOptimizationManifestIdentity,
};
use selected_instructions::SelectedInstructionPlanIdentity;
use semantic_vocabulary::{MachineId, WorkScheduleIdentity};
use terminal_psi::TerminalPsiIdentity;

use crate::instruction_selection::{SelectedInstructionError, ValidatedSelectedInstructions};
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::StagedOptimizedCustodyCore;

#[derive(Debug)]
pub struct StagedOptimizedSelectedInstructions {
    pub(crate) body: std::sync::Arc<physical_program::NativeBodyFacts>,
    pub(crate) register_environment: ValidatedTargetRegisterEnvironment,
    pub(crate) selections: OptimizationSelections,
    pub(crate) budget_per_pass: OptimizationWorkBudget,
    pub(crate) selected: ValidatedSelectedInstructions,
    pub(crate) custody: StagedOptimizedSelectionCustodyReceipt,
}

impl StagedOptimizedSelectedInstructions {
    pub fn body_facts(&self) -> &std::sync::Arc<physical_program::NativeBodyFacts> {
        &self.body
    }

    pub fn into_optimization_settings(
        self,
    ) -> (
        ValidatedTargetRegisterEnvironment,
        OptimizationSelections,
        OptimizationWorkBudget,
    ) {
        (
            self.register_environment,
            self.selections,
            self.budget_per_pass,
        )
    }

    pub fn selected_program(&self) -> selected_instructions::SelectedProgram {
        let receipt = self.selected.receipt();
        selected_instructions::SelectedProgram {
            plan: self.selected.shared_plan(),
            identity: receipt.identity(),
            optimization_unit: receipt.optimization_unit(),
            work_schedule: receipt.work_schedule(),
        }
    }

    pub const fn custody(&self) -> StagedOptimizedSelectionCustodyReceipt {
        self.custody
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StagedOptimizedSelectionCustodyReceipt {
    pub(crate) core: StagedOptimizedCustodyCore,
    pub(crate) legalization_validator: OptimizationValidatorIdentity,
    pub(crate) selected: SelectedInstructionPlanIdentity,
    pub(crate) function_count: usize,
}

impl StagedOptimizedSelectionCustodyReceipt {
    selected_instructions::staging_custody_core_accessors!();

    pub const fn selected(self) -> SelectedInstructionPlanIdentity {
        self.selected
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
    Installation(&'static str),
    BodyBindings(&'static str),
    RegisterEnvironment(register_environment::TargetRegisterEnvironmentValidationError),
    Binding(crate::input::InstallationBindingError),
    Selection(SelectedInstructionError),
}

impl std::fmt::Display for OptimizedSelectionPipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "optimized instruction selection failed: {self:?}")
    }
}

impl std::error::Error for OptimizedSelectionPipelineError {}

pub(crate) fn selection_custody_receipt(
    input: &legalized_operations::PreparedInstructionSelection,
    register_environment: &ValidatedTargetRegisterEnvironment,
    bound: &BoundExecutionPlan<'_>,
    selected: &ValidatedSelectedInstructions,
) -> StagedOptimizedSelectionCustodyReceipt {
    let plan = bound.plan();
    StagedOptimizedSelectionCustodyReceipt {
        core: StagedOptimizedCustodyCore {
            psi: plan.psi,
            target: plan.target,
            entry: plan.entry,
            optimization: input.optimization,
            projection: input.projection,
            manifest: input.manifest,
            optimization_unit: plan.optimization_unit,
            work_schedule: plan.work_schedule,
            register_environment: register_environment.identity(),
        },
        legalization_validator: bound.receipt().validator(),
        selected: selected.receipt().identity(),
        function_count: selected.plan().functions.len(),
    }
}
