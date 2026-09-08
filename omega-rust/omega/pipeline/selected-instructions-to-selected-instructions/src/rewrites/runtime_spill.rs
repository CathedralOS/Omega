//! Private, bit-preserving runtime-value storage. Each use gets its own reload;
//! no reload interval needs to survive an intervening call.

mod admission;
mod rewrite;
mod validation;

use std::sync::Arc;

use optimization_core::OptimizationUnitIdentity;
use selected_instructions::{SelectedInstructionPlan, SelectedInstructionPlanIdentity};
use semantic_vocabulary::FuelScheduleIdentity;

pub use rewrite::spill_selected_runtime_value;
pub use validation::validate_runtime_spill;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedRuntimeSpill {
    transformed: Arc<SelectedInstructionPlan>,
    receipt: RuntimeSpillReceipt,
}

impl ValidatedRuntimeSpill {
    pub fn transformed(&self) -> &SelectedInstructionPlan {
        &self.transformed
    }

    pub fn shared_transformed(&self) -> Arc<SelectedInstructionPlan> {
        Arc::clone(&self.transformed)
    }

    pub const fn receipt(&self) -> &RuntimeSpillReceipt {
        &self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeSpillReceipt {
    source_selected: SelectedInstructionPlanIdentity,
    transformed_selected: SelectedInstructionPlanIdentity,
    optimization_unit: OptimizationUnitIdentity,
    fuel_schedule: FuelScheduleIdentity,
}

impl RuntimeSpillReceipt {
    pub const fn source_selected(&self) -> SelectedInstructionPlanIdentity {
        self.source_selected
    }
    pub const fn transformed_selected(&self) -> SelectedInstructionPlanIdentity {
        self.transformed_selected
    }
    pub const fn optimization_unit(&self) -> OptimizationUnitIdentity {
        self.optimization_unit
    }
    pub const fn fuel_schedule(&self) -> FuelScheduleIdentity {
        self.fuel_schedule
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeSpillError {
    SourceMismatch,
    UnsupportedValue,
    UnsupportedControlFlow,
    UnsupportedUse,
    ConstraintMismatch,
    WorkBudgetExceeded,
    IdentityOverflow,
    ReplayMismatch,
}

impl std::fmt::Display for RuntimeSpillError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid runtime-value spill: {self:?}")
    }
}

impl std::error::Error for RuntimeSpillError {}

#[cfg(test)]
mod tests;
