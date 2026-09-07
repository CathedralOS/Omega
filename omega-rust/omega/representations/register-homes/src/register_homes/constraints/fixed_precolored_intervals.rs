//! Durable fixed precolored intervals data; not validation or allocation authority.

mod identity;
pub use identity::fixed_precolored_interval_plan_identity;

use optimization_core::{OptimizationUnitIdentity, OptimizationWorkBudget, OptimizationWorkUsage};
use register_model::{RegisterClassId, RegisterViewId, TargetRegisterEnvironmentIdentity};
use selected_instructions::{SelectedBlockId, VirtualRegisterId};
use semantic_vocabulary::{FuelScheduleIdentity, MachineId};

use crate::{AllocationLegalityIdentity, AllocatorAvailabilityIdentity};
use selected_instructions::{LiveRangeIdentity, LiveRangePoint, VirtualFixedConstraintSite};

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
