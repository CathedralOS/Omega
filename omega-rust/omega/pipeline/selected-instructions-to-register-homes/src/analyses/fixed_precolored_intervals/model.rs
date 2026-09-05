use optimization_core::{OptimizationUnitIdentity, OptimizationWorkBudget, OptimizationWorkUsage};
use register_model::{RegisterClassId, RegisterViewId, TargetRegisterEnvironmentIdentity};
use selected_instructions::{SelectedBlockId, VirtualRegisterId};
use semantic_vocabulary::{FuelScheduleIdentity, MachineId};

use crate::{
    AllocationLegalityIdentity, AllocatorAvailabilityIdentity, LiveRangeIdentity, LiveRangePoint,
    VirtualFixedConstraintSite,
};

pub use register_homes::FixedPrecoloredIntervalPlanIdentity;

pub use register_homes::FixedPrecoloredIntervalPolicy;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedPrecoloredIntervalPlan {
    pub ranges: LiveRangeIdentity,
    pub legality: AllocationLegalityIdentity,
    pub register_environment: TargetRegisterEnvironmentIdentity,
    pub allocator_availability: AllocatorAvailabilityIdentity,
    pub optimization_unit: OptimizationUnitIdentity,
    pub fuel_schedule: FuelScheduleIdentity,
    pub policy: FixedPrecoloredIntervalPolicy,
    pub budget: OptimizationWorkBudget,
    pub usage: OptimizationWorkUsage,
    pub functions: Vec<FunctionFixedPrecoloredIntervals>,
    pub structural_unit_functions: Vec<FunctionFixedPrecoloredIntervals>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionFixedPrecoloredIntervals {
    pub machine: MachineId,
    pub intervals: Vec<FixedPrecoloredInterval>,
}

/// A fixed selected constraint resolved to one exact physical view and one
/// half-open liveness phase. This is factual precoloring evidence, not a home.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FixedPrecoloredInterval {
    pub virtual_register: VirtualRegisterId,
    pub class: RegisterClassId,
    pub site: VirtualFixedConstraintSite,
    pub block: SelectedBlockId,
    pub start: LiveRangePoint,
    pub end: LiveRangePoint,
    pub view: RegisterViewId,
}

pub use register_homes::FixedPrecoloredIntervalValidationReceipt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedFixedPrecoloredIntervals {
    pub(crate) plan: FixedPrecoloredIntervalPlan,
    pub(crate) receipt: FixedPrecoloredIntervalValidationReceipt,
}

impl ValidatedFixedPrecoloredIntervals {
    pub const fn plan(&self) -> &FixedPrecoloredIntervalPlan {
        &self.plan
    }
    pub const fn receipt(&self) -> FixedPrecoloredIntervalValidationReceipt {
        self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FixedPrecoloredIntervalError {
    RootMismatch,
    FunctionMismatch {
        function: usize,
    },
    RegisterMismatch {
        function: usize,
        register: u32,
    },
    ConstraintPointMissing {
        function: usize,
        register: u32,
        point: u32,
    },
    ConstraintViewMismatch {
        function: usize,
        register: u32,
        view: u16,
    },
    UnsupportedEarlyClobberFixedConstraint {
        function: usize,
        register: u32,
        instruction: u32,
        operand: u16,
    },
    IntervalOverflow {
        function: usize,
        register: u32,
        point: u32,
    },
    WorkOverflow,
    BudgetExceeded {
        required: OptimizationWorkUsage,
        budget: OptimizationWorkBudget,
    },
    UsageMismatch,
    NonCanonicalFunctions,
}

impl std::fmt::Display for FixedPrecoloredIntervalError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "fixed/precolored interval analysis failed: {self:?}"
        )
    }
}

impl std::error::Error for FixedPrecoloredIntervalError {}
