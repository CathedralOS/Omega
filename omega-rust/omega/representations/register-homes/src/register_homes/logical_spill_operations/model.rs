use optimization_core::{OptimizationUnitIdentity, OptimizationWorkBudget, OptimizationWorkUsage};
use optimization_unit::ValueDefinitionSite;
use register_model::{RegisterClassId, RegisterViewId, TargetRegisterEnvironmentIdentity};
use selected_instructions::{
    LiveRangeIdentity, LiveRangePoint, SelectedBlockId, SelectedInstructionId,
    SelectedInstructionPlanIdentity, VirtualRegisterId, VirtualRegisterOrigin,
};
use semantic_vocabulary::{FuelScheduleIdentity, MachineId, ScalarType};

use crate::{AllocationLegalityIdentity, AllocatorAvailabilityIdentity, SpillChoiceIdentity};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LogicalSpillOperationIdentity(pub(crate) [u8; 32]);

impl LogicalSpillOperationIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Closed compiler-private planning policy. This is not an optimization name,
/// cost model, stack policy, or frame-layout policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LogicalSpillOperationPolicy {
    SelectedActiveResidentInstructionResultU64StoreBeforePressureReloadBeforeFirstFutureFlexibleUseV1,
}

/// Target-neutral recovery obligations for validated allocation pressure.
///
/// This artifact allocates only logical namespaces. It grants no physical
/// stack slot, offset, instruction, frame, unwind, trap, or publication
/// authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogicalSpillOperationPlan {
    pub selected: SelectedInstructionPlanIdentity,
    pub ranges: LiveRangeIdentity,
    pub legality: AllocationLegalityIdentity,
    pub spill_choices: SpillChoiceIdentity,
    pub register_environment: TargetRegisterEnvironmentIdentity,
    pub allocator_availability: AllocatorAvailabilityIdentity,
    pub optimization_unit: OptimizationUnitIdentity,
    pub fuel_schedule: FuelScheduleIdentity,
    pub policy: LogicalSpillOperationPolicy,
    pub budget: OptimizationWorkBudget,
    pub usage: OptimizationWorkUsage,
    pub functions: Vec<FunctionLogicalSpillOperations>,
}

impl LogicalSpillOperationPlan {
    /// Canonical transport only. Decoding does not grant spill authority;
    /// independent replay against the validated roots remains mandatory.
    pub fn encode(&self) -> Vec<u8> {
        super::codec::encode(self)
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, LogicalSpillOperationDecodeError> {
        super::codec::decode(encoded)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionLogicalSpillOperations {
    pub machine: MachineId,
    pub action: Option<LogicalSpillAction>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LogicalSpillStorageId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LogicalReloadValueId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LogicalSpillStorageClass {
    NonAddressUnsignedU64V1,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogicalSpillAction {
    pub block: SelectedBlockId,
    pub pressure_point: LiveRangePoint,
    pub incoming: VirtualRegisterId,
    pub incoming_class: RegisterClassId,
    pub victim: VirtualRegisterId,
    pub victim_class: RegisterClassId,
    pub victim_scalar_type: ScalarType,
    pub victim_origin: VirtualRegisterOrigin,
    pub victim_definition_site: ValueDefinitionSite,
    pub current_view: RegisterViewId,
    pub reclaimed_view: RegisterViewId,
    pub storage: LogicalSpillStorage,
    pub store: LogicalSpillStore,
    pub reload: LogicalSpillReload,
    pub rewrites: Vec<LogicalSpillUseRewrite>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LogicalSpillStorage {
    pub id: LogicalSpillStorageId,
    pub class: LogicalSpillStorageClass,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LogicalSpillStore {
    pub before_instruction: SelectedInstructionId,
    pub source: VirtualRegisterId,
    pub storage: LogicalSpillStorageId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LogicalSpillReload {
    pub before_instruction: SelectedInstructionId,
    pub storage: LogicalSpillStorageId,
    pub result: LogicalReloadValueId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct LogicalSpillUseRewrite {
    pub block: SelectedBlockId,
    pub point: LiveRangePoint,
    pub instruction: SelectedInstructionId,
    pub operand: u16,
    pub result: LogicalReloadValueId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogicalSpillOperationDecodeError {
    Truncated,
    WrongMagic,
    UnsupportedVersion(u32),
    UnknownPolicy(u8),
    UnknownOption(u8),
    UnknownStorageClass(u8),
    UnknownScalarType(u8),
    UnknownIntegerCarrier(u8),
    UnknownIntegerSign(u8),
    InvalidIntegerType,
    UnknownOrigin(u8),
    UnknownDefinitionSite(u8),
    InvalidMachineId(u64),
    InvalidBlockId(u64),
    InvalidValueId(u64),
    InvalidPlaceId(u64),
    InvalidFuelSchedule(u32),
    InvalidBudget,
    InvalidUsage,
    LengthOverflow,
    IdentityMismatch,
    TrailingBytes,
}

impl std::fmt::Display for LogicalSpillOperationDecodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid Terminal logical-spill encoding: {self:?}"
        )
    }
}

impl std::error::Error for LogicalSpillOperationDecodeError {}
