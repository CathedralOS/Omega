use optimization_core::{OptimizationUnitIdentity, OptimizationWorkBudget, OptimizationWorkUsage};
use optimization_unit::ValueDefinitionSite;
use register_model::{RegisterClassId, RegisterViewId, TargetRegisterEnvironmentIdentity};
use selected_instructions::{
    SelectedBlockId, SelectedInstructionId, SelectedInstructionPlanIdentity, VirtualRegisterId,
    VirtualRegisterOrigin,
};
use semantic_vocabulary::{FuelScheduleIdentity, MachineId, ScalarType};

use crate::{
    AllocationLegalityIdentity, AllocatorAvailabilityIdentity, LiveRangeIdentity, LiveRangePoint,
    SpillChoiceIdentity,
};

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
pub struct LogicalSpillOperationValidationReceipt {
    pub(crate) identity: LogicalSpillOperationIdentity,
    pub(crate) selected: SelectedInstructionPlanIdentity,
    pub(crate) ranges: LiveRangeIdentity,
    pub(crate) legality: AllocationLegalityIdentity,
    pub(crate) spill_choices: SpillChoiceIdentity,
    pub(crate) register_environment: TargetRegisterEnvironmentIdentity,
    pub(crate) allocator_availability: AllocatorAvailabilityIdentity,
    pub(crate) optimization_unit: OptimizationUnitIdentity,
    pub(crate) fuel_schedule: FuelScheduleIdentity,
    pub(crate) policy: LogicalSpillOperationPolicy,
    pub(crate) usage: OptimizationWorkUsage,
    pub(crate) function_count: usize,
    pub(crate) planned_function_count: usize,
    pub(crate) store_count: usize,
    pub(crate) reload_count: usize,
    pub(crate) rewritten_use_count: usize,
}

impl LogicalSpillOperationValidationReceipt {
    pub const fn identity(self) -> LogicalSpillOperationIdentity {
        self.identity
    }
    pub const fn selected(self) -> SelectedInstructionPlanIdentity {
        self.selected
    }
    pub const fn ranges(self) -> LiveRangeIdentity {
        self.ranges
    }
    pub const fn legality(self) -> AllocationLegalityIdentity {
        self.legality
    }
    pub const fn spill_choices(self) -> SpillChoiceIdentity {
        self.spill_choices
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
    pub const fn policy(self) -> LogicalSpillOperationPolicy {
        self.policy
    }
    pub const fn usage(self) -> OptimizationWorkUsage {
        self.usage
    }
    pub const fn function_count(self) -> usize {
        self.function_count
    }
    pub const fn planned_function_count(self) -> usize {
        self.planned_function_count
    }
    pub const fn store_count(self) -> usize {
        self.store_count
    }
    pub const fn reload_count(self) -> usize {
        self.reload_count
    }
    pub const fn rewritten_use_count(self) -> usize {
        self.rewritten_use_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedLogicalSpillOperations {
    pub(crate) plan: LogicalSpillOperationPlan,
    pub(crate) receipt: LogicalSpillOperationValidationReceipt,
}

impl ValidatedLogicalSpillOperations {
    pub const fn plan(&self) -> &LogicalSpillOperationPlan {
        &self.plan
    }
    pub const fn receipt(&self) -> LogicalSpillOperationValidationReceipt {
        self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogicalSpillOperationError {
    RootMismatch,
    UnsupportedPolicy,
    WorkOverflow,
    BudgetExceeded {
        required: OptimizationWorkUsage,
        budget: OptimizationWorkBudget,
    },
    FunctionMismatch {
        function: usize,
    },
    UnsupportedVictimRole {
        function: usize,
        register: u32,
    },
    UnsupportedScalarType {
        function: usize,
        register: u32,
    },
    UnsupportedOrigin {
        function: usize,
        register: u32,
    },
    UnsupportedRangeShape {
        function: usize,
        register: u32,
    },
    IncomingDefinitionMismatch {
        function: usize,
        register: u32,
    },
    FutureFixedUse {
        function: usize,
        register: u32,
    },
    NoFutureUse {
        function: usize,
        register: u32,
    },
    FutureUseMismatch {
        function: usize,
        register: u32,
    },
    IdentifierOverflow {
        function: usize,
    },
    NonCanonicalStorageIds {
        function: usize,
    },
    DecisionMismatch {
        function: usize,
    },
    UsageMismatch,
}

impl std::fmt::Display for LogicalSpillOperationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "Terminal logical spill planning failed: {self:?}"
        )
    }
}

impl std::error::Error for LogicalSpillOperationError {}

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
