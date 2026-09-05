use optimization_core::{OptimizationUnitIdentity, OptimizationWorkBudget, OptimizationWorkUsage};
use register_model::{RegisterClassId, RegisterViewId, TargetRegisterEnvironmentIdentity};
use selected_instructions::{SelectedBlockId, VirtualRegisterId};
use semantic_vocabulary::{FuelScheduleIdentity, MachineId};

use crate::{
    AllocationLegalityIdentity, AllocatorAvailabilityIdentity, LiveRangeIdentity, LiveRangePoint,
    VirtualFixedConstraintSite,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FixedPrecoloredIntervalPlanIdentity(pub(crate) [u8; 32]);

impl FixedPrecoloredIntervalPlanIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FixedPrecoloredIntervalPolicy {
    /// Every selected fixed constraint occupies exactly its authenticated
    /// liveness phase, represented as `[point, point + 1)`.
    FixedConstraintPointIntervalsV1,
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FixedPrecoloredIntervalValidationReceipt {
    pub(crate) identity: FixedPrecoloredIntervalPlanIdentity,
    pub(crate) ranges: LiveRangeIdentity,
    pub(crate) legality: AllocationLegalityIdentity,
    pub(crate) register_environment: TargetRegisterEnvironmentIdentity,
    pub(crate) allocator_availability: AllocatorAvailabilityIdentity,
    pub(crate) optimization_unit: OptimizationUnitIdentity,
    pub(crate) fuel_schedule: FuelScheduleIdentity,
    pub(crate) policy: FixedPrecoloredIntervalPolicy,
    pub(crate) usage: OptimizationWorkUsage,
    pub(crate) function_count: usize,
    pub(crate) structural_unit_function_count: usize,
    pub(crate) inspected_register_count: usize,
    pub(crate) interval_count: usize,
    pub(crate) entry_interval_count: usize,
    pub(crate) operand_interval_count: usize,
}

impl FixedPrecoloredIntervalValidationReceipt {
    pub const fn identity(self) -> FixedPrecoloredIntervalPlanIdentity {
        self.identity
    }
    pub const fn ranges(self) -> LiveRangeIdentity {
        self.ranges
    }
    pub const fn legality(self) -> AllocationLegalityIdentity {
        self.legality
    }
    pub const fn register_environment(self) -> TargetRegisterEnvironmentIdentity {
        self.register_environment
    }
    pub const fn allocator_availability(self) -> AllocatorAvailabilityIdentity {
        self.allocator_availability
    }
    pub const fn optimization_unit(self) -> OptimizationUnitIdentity {
        self.optimization_unit
    }
    pub const fn fuel_schedule(self) -> FuelScheduleIdentity {
        self.fuel_schedule
    }
    pub const fn policy(self) -> FixedPrecoloredIntervalPolicy {
        self.policy
    }
    pub const fn usage(self) -> OptimizationWorkUsage {
        self.usage
    }
    pub const fn function_count(self) -> usize {
        self.function_count
    }
    pub const fn structural_unit_function_count(self) -> usize {
        self.structural_unit_function_count
    }
    pub const fn inspected_register_count(self) -> usize {
        self.inspected_register_count
    }
    pub const fn interval_count(self) -> usize {
        self.interval_count
    }
    pub const fn entry_interval_count(self) -> usize {
        self.entry_interval_count
    }
    pub const fn operand_interval_count(self) -> usize {
        self.operand_interval_count
    }
}

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
