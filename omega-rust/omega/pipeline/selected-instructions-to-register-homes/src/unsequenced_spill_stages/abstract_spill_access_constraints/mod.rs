//! Optimizer module role: executable entrance. Abstract spill-access constraints.
//!
//! This boundary orders compiler-private abstract accesses and records their
//! data, declared-barrier, and overlapping-slice dependencies. It grants no
//! executable operation, address, frame, fault, opcode, or publication authority.

mod compute;
mod identity;
mod replay;
mod validate;

use crate::unsequenced_spill_stages::{
    AbstractSpillMemoryEffectPlanIdentity, GeneralizedSpillActionId, SpillPseudoInstructionId,
};
use crate::{AllocatorAvailabilityIdentity, LiveRangePoint};
pub use error::*;
pub use identity::abstract_spill_access_constraint_plan_identity;
use optimization_core::{OptimizationUnitIdentity, OptimizationWorkBudget, OptimizationWorkUsage};
pub use register_homes::AbstractSpillAccessConstraintPlanIdentity;
use register_model::TargetRegisterEnvironmentIdentity;
use selected_instructions::{SelectedBlockId, SelectedInstructionId};
use semantic_vocabulary::{FuelScheduleIdentity, MachineId};
pub use validate::validate_abstract_spill_access_constraints;

pub fn constrain_abstract_spill_accesses(
    source: &crate::unsequenced_spill_stages::ValidatedAbstractSpillMemoryEffects,
    policy: AbstractSpillAccessConstraintPolicy,
    budget: optimization_core::OptimizationWorkBudget,
) -> Result<ValidatedAbstractSpillAccessConstraints, AbstractSpillAccessConstraintError> {
    let plan = compute::compute(source, policy, budget)?;
    validate_abstract_spill_access_constraints(source, plan)
}

mod error;

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
