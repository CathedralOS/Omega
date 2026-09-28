use optimization_core::{OptimizationUnitIdentity, OptimizationWorkBudget, OptimizationWorkUsage};
use register_model::TargetRegisterEnvironmentIdentity;
use selected_instructions::{LiveRangePoint, SelectedBlockId};
use semantic_vocabulary::{FuelScheduleIdentity, MachineId};

use crate::{
    AllocatorAvailabilityIdentity, LogicalSpillOperationIdentity, LogicalSpillStorageClass,
    LogicalSpillStorageId,
};

/// Identity of a canonical, independently replayable stack-slot coloring.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StackSlotColoringIdentity(pub(crate) [u8; 32]);

impl StackSlotColoringIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Closed target-neutral policy. Lifetimes are closed, so touching endpoints conflict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StackSlotColoringPolicy {
    BlockLocalNonAddressUnsignedU64ClosedIntervalFirstFitV1,
}

/// Target-neutral assignments relative to the beginning of an unspecified spill area.
///
/// This artifact grants no final frame, stack-pointer offset, instruction,
/// unwind, ABI-layout, or publication authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StackSlotColoringPlan {
    pub logical_spill_operations: LogicalSpillOperationIdentity,
    pub register_environment: TargetRegisterEnvironmentIdentity,
    pub allocator_availability: AllocatorAvailabilityIdentity,
    pub optimization_unit: OptimizationUnitIdentity,
    pub fuel_schedule: FuelScheduleIdentity,
    pub policy: StackSlotColoringPolicy,
    pub budget: OptimizationWorkBudget,
    pub usage: OptimizationWorkUsage,
    pub functions: Vec<FunctionStackSlotColoring>,
}

impl StackSlotColoringPlan {
    /// Canonical transport only. Independent replay is still required for authority.
    pub fn encode(&self) -> Vec<u8> {
        super::codec::encode(self)
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, StackSlotColoringDecodeError> {
        super::codec::decode(encoded)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionStackSlotColoring {
    pub machine: MachineId,
    pub assignments: Vec<StackSlotAssignment>,
    /// Bytes required from a future spill area. This is not a frame size.
    pub spill_area_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StackSlotAssignment {
    pub storage: LogicalSpillStorageId,
    pub class: LogicalSpillStorageClass,
    pub block: SelectedBlockId,
    pub live_from: LiveRangePoint,
    pub live_through: LiveRangePoint,
    pub size_bytes: u64,
    pub alignment_bytes: u64,
    /// Byte offset relative to the beginning of an as-yet-unlaid-out spill area.
    pub spill_area_offset: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StackSlotColoringDecodeError {
    Truncated,
    WrongMagic,
    UnsupportedVersion(u32),
    UnknownPolicy(u8),
    UnknownStorageClass(u8),
    InvalidMachineId(u64),
    InvalidBudget,
    InvalidUsage,
    InvalidFuelSchedule(u32),
    LengthOverflow,
    IdentityMismatch,
    TrailingBytes,
}

impl std::fmt::Display for StackSlotColoringDecodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid stack-slot-coloring encoding: {self:?}")
    }
}

impl std::error::Error for StackSlotColoringDecodeError {}
