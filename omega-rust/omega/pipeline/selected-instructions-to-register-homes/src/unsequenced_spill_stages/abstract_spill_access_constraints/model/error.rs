use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};

use crate::{GeneralizedSpillActionId, SpillPseudoInstructionId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbstractSpillAccessConstraintError {
    RootMismatch,
    UnsupportedPolicy,
    DuplicateAccess {
        function: usize,
        pseudo: SpillPseudoInstructionId,
    },
    InvalidAccessOrder {
        function: usize,
    },
    InvalidGeometry {
        function: usize,
        pseudo: SpillPseudoInstructionId,
    },
    DuplicateWrite {
        function: usize,
        storage: GeneralizedSpillActionId,
    },
    MissingWrite {
        function: usize,
        storage: GeneralizedSpillActionId,
    },
    InvalidBeforeReload {
        function: usize,
        pseudo: SpillPseudoInstructionId,
    },
    WorkOverflow,
    NonCanonicalFunctions,
    UsageMismatch,
    BudgetExceeded {
        required: OptimizationWorkUsage,
        budget: OptimizationWorkBudget,
    },
}

impl std::fmt::Display for AbstractSpillAccessConstraintError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "abstract spill-access constraint derivation failed: {self:?}"
        )
    }
}

impl std::error::Error for AbstractSpillAccessConstraintError {}
