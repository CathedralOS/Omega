//! Unchecked pressure-victim evidence and canonical transport.

mod codec;
mod identity;
pub use identity::spill_choice_identity;

use crate::{AllocationLegalityIdentity, AllocatorAvailabilityIdentity};
use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use register_model::{RegisterClassId, RegisterViewId, TargetRegisterEnvironmentIdentity};
use selected_instructions::{
    LiveRangeIdentity, LiveRangePoint, SelectedBlockId, VirtualRegisterId,
};
use semantic_vocabulary::MachineId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SpillChoiceIdentity(pub(crate) [u8; 32]);

impl SpillChoiceIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Stable structural policy for the first locally witnessed pressure point.
/// This is not an optimization level or a target cost model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpillChoicePolicy {
    SingleBlockFarthestEndThenHighestVregV1,
}

/// Deterministic recovery-victim evidence. Despite the historical “spill”
/// name, this artifact grants no spill/reload, rematerialization, stack-slot,
/// frame, instruction-emission, or publication authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpillChoicePlan {
    pub legality: AllocationLegalityIdentity,
    pub ranges: LiveRangeIdentity,
    pub register_environment: TargetRegisterEnvironmentIdentity,
    pub allocator_availability: AllocatorAvailabilityIdentity,
    pub policy: SpillChoicePolicy,
    pub budget: OptimizationWorkBudget,
    pub usage: OptimizationWorkUsage,
    pub functions: Vec<FunctionSpillChoices>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionSpillChoices {
    pub machine: MachineId,
    /// The first pressure point only. `None` proves this bounded greedy walk
    /// encountered no pressure; it does not prove globally optimal coloring.
    pub choice: Option<SpillChoice>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpillChoice {
    pub block: SelectedBlockId,
    pub point: LiveRangePoint,
    pub incoming: VirtualRegisterId,
    pub incoming_class: RegisterClassId,
    pub incoming_common_candidates: Vec<RegisterViewId>,
    pub active_residents: Vec<PressureResident>,
    pub contenders: Vec<PressureContender>,
    pub selected_victim: VirtualRegisterId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PressureResident {
    pub virtual_register: VirtualRegisterId,
    pub class: RegisterClassId,
    pub start: LiveRangePoint,
    pub exclusive_end: LiveRangePoint,
    pub view: RegisterViewId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PressureContender {
    pub virtual_register: VirtualRegisterId,
    pub exclusive_end: LiveRangePoint,
    /// `None` denotes keeping an incoming value out of the current homes.
    /// `Some(view)` is the lowest legal incoming view recovered by evicting
    /// the named active resident. It is evidence, not permission to evict.
    pub reclaimed_view: Option<RegisterViewId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpillChoiceDecodeError {
    Truncated,
    WrongMagic,
    UnsupportedVersion(u32),
    UnknownPolicy(u8),
    UnknownOption(u8),
    InvalidBudget,
    InvalidUsage,
    InvalidMachineId(u64),
    LengthOverflow,
    IdentityMismatch,
    TrailingBytes,
}

impl std::fmt::Display for SpillChoiceDecodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid Terminal spill-choice encoding: {self:?}"
        )
    }
}

impl std::error::Error for SpillChoiceDecodeError {}
