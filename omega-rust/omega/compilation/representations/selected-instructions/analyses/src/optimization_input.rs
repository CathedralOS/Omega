//! The selected program and the admission it arrives with.

use optimization_core::{OptimizationSelections, OptimizationWorkBudget};
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{SelectedProgram, StagedOptimizedCustodyCore};

/// What instruction selection hands this stage: the selected program, the
/// custody core its receipt carries, and the register environment, optimizer
/// selections and per-pass budget admitted with it.
#[derive(Debug)]
pub struct SelectedOptimizationInput {
    pub program: SelectedProgram,
    pub custody: StagedOptimizedCustodyCore,
    pub register_environment: ValidatedTargetRegisterEnvironment,
    pub selections: OptimizationSelections,
    pub budget_per_pass: OptimizationWorkBudget,
}

impl SelectedOptimizationInput {
    pub const fn selected(&self) -> &SelectedProgram {
        &self.program
    }

    pub const fn register_environment(&self) -> &ValidatedTargetRegisterEnvironment {
        &self.register_environment
    }

    pub const fn selections(&self) -> &OptimizationSelections {
        &self.selections
    }

    pub const fn budget_per_pass(&self) -> OptimizationWorkBudget {
        self.budget_per_pass
    }
}
