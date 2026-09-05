use optimization_core::{OptimizationUnitIdentity, OptimizationWorkBudget, OptimizationWorkUsage};
use register_model::TargetRegisterEnvironmentIdentity;
use selected_instructions::{SelectedBlockId, SelectedInstructionId};
use semantic_vocabulary::{FuelScheduleIdentity, MachineId};

use crate::{
    AbstractSpillMemoryEffectPlanIdentity, AllocatorAvailabilityIdentity, GeneralizedSpillActionId,
    LiveRangePoint, SpillPseudoInstructionId,
};

mod error;

pub use error::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AbstractSpillAccessConstraintPlanIdentity(pub(crate) [u8; 32]);

impl AbstractSpillAccessConstraintPlanIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AbstractSpillAccessConstraintPolicy {
    BlockLocalDataBarrierAndOverlapV1,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbstractSpillAccessConstraintPlan {
    pub abstract_spill_memory_effects: AbstractSpillMemoryEffectPlanIdentity,
    pub register_environment: TargetRegisterEnvironmentIdentity,
    pub allocator_availability: AllocatorAvailabilityIdentity,
    pub optimization_unit: OptimizationUnitIdentity,
    pub fuel_schedule: FuelScheduleIdentity,
    pub policy: AbstractSpillAccessConstraintPolicy,
    pub budget: OptimizationWorkBudget,
    pub usage: OptimizationWorkUsage,
    pub functions: Vec<FunctionAbstractSpillAccessConstraints>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionAbstractSpillAccessConstraints {
    pub machine: MachineId,
    /// Abstract extent only; no frame is allocated by this artifact.
    pub spill_area_bytes: u64,
    pub placements: Vec<AbstractSpillAccessPlacement>,
    pub dependencies: Vec<AbstractSpillAccessDependency>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AbstractSpillAccessKind {
    Write,
    Read,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbstractSpillAccessPlacement {
    pub pseudo: SpillPseudoInstructionId,
    pub block: SelectedBlockId,
    /// Dense only within `block`; it makes no cross-block execution claim.
    pub block_ordinal: u32,
    pub point: LiveRangePoint,
    pub before_instruction: SelectedInstructionId,
    pub kind: AbstractSpillAccessKind,
    pub storage: GeneralizedSpillActionId,
    /// Relative to the abstract spill-area origin, never SP or FP.
    pub spill_area_offset: u64,
    pub size_bytes: u64,
    pub alignment_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AbstractSpillAccessDependencyReason {
    StoredValue {
        storage: GeneralizedSpillActionId,
    },
    DeclaredBeforeReload,
    OverlappingAbstractSlice {
        spill_area_offset: u64,
        size_bytes: u64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AbstractSpillAccessDependency {
    pub before: SpillPseudoInstructionId,
    pub after: SpillPseudoInstructionId,
    pub reason: AbstractSpillAccessDependencyReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbstractSpillAccessConstraintReceipt {
    pub(crate) identity: AbstractSpillAccessConstraintPlanIdentity,
    pub(crate) abstract_spill_memory_effects: AbstractSpillMemoryEffectPlanIdentity,
    pub(crate) register_environment: TargetRegisterEnvironmentIdentity,
    pub(crate) allocator_availability: AllocatorAvailabilityIdentity,
    pub(crate) optimization_unit: OptimizationUnitIdentity,
    pub(crate) fuel_schedule: FuelScheduleIdentity,
    pub(crate) usage: OptimizationWorkUsage,
    pub(crate) function_count: usize,
    pub(crate) placement_count: usize,
    pub(crate) dependency_count: usize,
    pub(crate) stored_value_dependency_count: usize,
    pub(crate) declared_barrier_count: usize,
    pub(crate) overlapping_slice_dependency_count: usize,
    pub(crate) max_spill_area_bytes: u64,
}

impl AbstractSpillAccessConstraintReceipt {
    pub const fn identity(self) -> AbstractSpillAccessConstraintPlanIdentity {
        self.identity
    }
    pub const fn abstract_spill_memory_effects(self) -> AbstractSpillMemoryEffectPlanIdentity {
        self.abstract_spill_memory_effects
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
    pub const fn usage(self) -> OptimizationWorkUsage {
        self.usage
    }
    pub const fn function_count(self) -> usize {
        self.function_count
    }
    pub const fn placement_count(self) -> usize {
        self.placement_count
    }
    pub const fn dependency_count(self) -> usize {
        self.dependency_count
    }
    pub const fn stored_value_dependency_count(self) -> usize {
        self.stored_value_dependency_count
    }
    pub const fn declared_barrier_count(self) -> usize {
        self.declared_barrier_count
    }
    pub const fn overlapping_slice_dependency_count(self) -> usize {
        self.overlapping_slice_dependency_count
    }
    pub const fn max_spill_area_bytes(self) -> u64 {
        self.max_spill_area_bytes
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedAbstractSpillAccessConstraints {
    pub(crate) plan: AbstractSpillAccessConstraintPlan,
    pub(crate) receipt: AbstractSpillAccessConstraintReceipt,
}

impl ValidatedAbstractSpillAccessConstraints {
    pub const fn plan(&self) -> &AbstractSpillAccessConstraintPlan {
        &self.plan
    }
    pub const fn receipt(&self) -> AbstractSpillAccessConstraintReceipt {
        self.receipt
    }
}
