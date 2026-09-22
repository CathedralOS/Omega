//! Optimizer module role: executable entrance. Selected-CFG liveness compute -> independent validation entrance.

use crate::ValidatedSelectedAnalysis;
pub(crate) mod compute;
pub(crate) mod edge_values;
pub(crate) mod validate;

#[cfg(test)]
pub(crate) mod tests;

use optimization_core::OptimizationUnitIdentity;
use selected_instructions::{LivenessIdentity, LivenessPlan, SelectedInstructionPlanIdentity};
use semantic_vocabulary::FuelScheduleIdentity;
pub use validate::validate_liveness;

#[cfg(test)]
pub(crate) use compute::FUNCTION_COMPUTATIONS as LIVENESS_FUNCTION_COMPUTATIONS;

/// Compute and independently replay bounded selected-CFG liveness facts.
/// The result grants no interval, allocation, emission, or publication
/// authority.
pub fn analyze_liveness<S: ValidatedSelectedAnalysis>(
    selected: &S,
) -> Result<ValidatedLiveness, LivenessError> {
    let plan = compute::compute_terminal_liveness(selected)?;
    validate_liveness(selected, plan)
}

/// Reuse unchanged function computations, then independently replay the entire
/// result. Prior facts are candidates, never authority for the new receipt.
pub fn analyze_liveness_reusing(
    previous: &impl ValidatedSelectedAnalysis,
    previous_liveness: &ValidatedLiveness,
    selected: &impl ValidatedSelectedAnalysis,
) -> Result<ValidatedLiveness, LivenessError> {
    let plan = compute::compute_terminal_liveness_reusing(previous, previous_liveness, selected)?;
    validate_liveness(selected, plan)
}

mod staging;
pub use staging::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LivenessValidationReceipt {
    pub(crate) identity: LivenessIdentity,
    pub(crate) selected: SelectedInstructionPlanIdentity,
    pub(crate) optimization_unit: OptimizationUnitIdentity,
    pub(crate) fuel_schedule: FuelScheduleIdentity,
    pub(crate) function_count: usize,
    pub(crate) block_count: usize,
    pub(crate) virtual_register_count: usize,
    pub(crate) instruction_count: usize,
    pub(crate) successor_count: usize,
    pub(crate) tied_pair_count: usize,
    pub(crate) early_clobber_count: usize,
}

impl LivenessValidationReceipt {
    pub const fn identity(self) -> LivenessIdentity {
        self.identity
    }

    pub const fn selected(self) -> SelectedInstructionPlanIdentity {
        self.selected
    }

    pub const fn optimization_unit(self) -> OptimizationUnitIdentity {
        self.optimization_unit
    }

    pub const fn fuel_schedule(self) -> FuelScheduleIdentity {
        self.fuel_schedule
    }

    pub const fn function_count(self) -> usize {
        self.function_count
    }

    pub const fn block_count(self) -> usize {
        self.block_count
    }

    pub const fn virtual_register_count(self) -> usize {
        self.virtual_register_count
    }

    pub const fn instruction_count(self) -> usize {
        self.instruction_count
    }

    pub const fn successor_count(self) -> usize {
        self.successor_count
    }

    pub const fn tied_pair_count(self) -> usize {
        self.tied_pair_count
    }

    pub const fn early_clobber_count(self) -> usize {
        self.early_clobber_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedLiveness {
    pub(crate) plan: std::sync::Arc<LivenessPlan>,
    pub(crate) receipt: LivenessValidationReceipt,
}

impl ValidatedLiveness {
    pub fn plan(&self) -> &LivenessPlan {
        &self.plan
    }

    pub const fn receipt(&self) -> LivenessValidationReceipt {
        self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LivenessError {
    RootMismatch,
    UnsupportedUseDef {
        function: usize,
        instruction: u32,
        operand: u16,
    },
    UnsupportedTiedOperand {
        function: usize,
        instruction: u32,
        operand: u16,
    },
    UnsupportedEarlyClobber {
        function: usize,
        instruction: u32,
        operand: u16,
    },
    FunctionMismatch {
        function: usize,
    },
    BlockMismatch {
        function: usize,
        block: u32,
    },
    InstructionMismatch {
        function: usize,
        instruction: u32,
    },
    SuccessorMismatch {
        function: usize,
        block: u32,
        ordinal: u8,
    },
    NonCanonicalSet {
        function: usize,
        instruction: Option<u32>,
    },
    NonDensePositions {
        function: usize,
    },
    TransferMismatch {
        function: usize,
        instruction: u32,
    },
    FixedConstraintMismatch {
        function: usize,
    },
}

impl std::fmt::Display for LivenessError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "Terminal selected liveness failed: {self:?}")
    }
}

impl std::error::Error for LivenessError {}
