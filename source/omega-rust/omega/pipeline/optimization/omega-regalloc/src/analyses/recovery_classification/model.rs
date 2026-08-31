use omega_optimization_core::{
    OptimizationUnitIdentity, OptimizationWorkBudget, OptimizationWorkUsage,
};
use omega_optimization_unit::ValueDefinitionSite;
use omega_register_model::{RegisterClassId, RegisterViewId, TargetRegisterEnvironmentIdentity};
use omega_selected_instructions::{
    SelectedBlockId, SelectedInstructionId, SelectedInstructionPlanIdentity,
    SelectedInstructionProvenance, VirtualRegisterId, VirtualRegisterOrigin,
};
use psi_core::{FuelScheduleIdentity, IntegerValue, MachineId, ScalarType, ValueId};

use crate::{
    AllocationLegalityIdentity, AllocatorAvailabilityIdentity, LiveRangeIdentity, LiveRangePoint,
    SpillChoiceIdentity,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RecoveryClassificationIdentity(pub(crate) [u8; 32]);

impl RecoveryClassificationIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Named, deliberately bounded policy for classifying the already selected
/// pressure victim. This is not a spill, rematerialization, or cost policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RecoveryClassificationPolicy {
    SelectedVictimImmediateU64EligibilityV1,
}

/// Semantic eligibility evidence for the first selected local pressure victim.
///
/// This artifact does not choose or insert recovery code. In particular, it
/// grants no storage, stack-slot, frame-layout, instruction-emission, logical-
/// fuel, or publication authority. A future materialization stage must join
/// target frame policy and independently validate any concrete transformation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryClassificationPlan {
    pub selected: SelectedInstructionPlanIdentity,
    pub spill_choices: SpillChoiceIdentity,
    pub ranges: LiveRangeIdentity,
    pub legality: AllocationLegalityIdentity,
    pub register_environment: TargetRegisterEnvironmentIdentity,
    pub allocator_availability: AllocatorAvailabilityIdentity,
    pub optimization_unit: OptimizationUnitIdentity,
    pub fuel_schedule: FuelScheduleIdentity,
    pub policy: RecoveryClassificationPolicy,
    pub budget: OptimizationWorkBudget,
    pub usage: OptimizationWorkUsage,
    pub functions: Vec<FunctionRecoveryClassification>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionRecoveryClassification {
    pub machine: MachineId,
    /// Exactly one row when the rooted spill-choice function selected a victim;
    /// otherwise `None`.
    pub classification: Option<PressureRecoveryClassification>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PressureRecoveryClassification {
    pub block: SelectedBlockId,
    pub point: LiveRangePoint,
    pub victim: VirtualRegisterId,
    pub role: RecoveryVictimRole,
    pub scalar_type: ScalarType,
    pub class: RegisterClassId,
    pub origin: VirtualRegisterOrigin,
    pub definition_site: ValueDefinitionSite,
    pub classification: RecoveryClassification,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryVictimRole {
    Incoming,
    ActiveResident {
        current_view: RegisterViewId,
        reclaimed_view: RegisterViewId,
    },
}

/// A semantic classification only. Even an admitted candidate is not
/// permission to recompute the value or alter its logical charge placement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecoveryClassification {
    ImmediateU64RematerializationCandidate {
        defining_instruction: SelectedInstructionId,
        source_value: ValueId,
        value: IntegerValue,
        provenance: SelectedInstructionProvenance,
        future_uses: Vec<RecoveryFutureUse>,
    },
    NoAdmittedRecovery {
        reason: NoAdmittedRecoveryReason,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct RecoveryFutureUse {
    pub block: SelectedBlockId,
    pub point: LiveRangePoint,
    pub instruction: SelectedInstructionId,
    pub operand: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NoAdmittedRecoveryReason {
    UnsupportedScalarType,
    EntryParameter,
    UnsupportedRangeShape,
    FutureFixedUse,
    NonMaterializeDefinition,
    ProofBearingDefinition,
    NoFutureUse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecoveryClassificationValidationReceipt {
    pub(crate) identity: RecoveryClassificationIdentity,
    pub(crate) selected: SelectedInstructionPlanIdentity,
    pub(crate) spill_choices: SpillChoiceIdentity,
    pub(crate) ranges: LiveRangeIdentity,
    pub(crate) legality: AllocationLegalityIdentity,
    pub(crate) register_environment: TargetRegisterEnvironmentIdentity,
    pub(crate) allocator_availability: AllocatorAvailabilityIdentity,
    pub(crate) optimization_unit: OptimizationUnitIdentity,
    pub(crate) fuel_schedule: FuelScheduleIdentity,
    pub(crate) policy: RecoveryClassificationPolicy,
    pub(crate) usage: OptimizationWorkUsage,
    pub(crate) function_count: usize,
    pub(crate) classification_count: usize,
    pub(crate) immediate_candidate_count: usize,
}

impl RecoveryClassificationValidationReceipt {
    pub const fn identity(self) -> RecoveryClassificationIdentity {
        self.identity
    }
    pub const fn selected(self) -> SelectedInstructionPlanIdentity {
        self.selected
    }
    pub const fn spill_choices(self) -> SpillChoiceIdentity {
        self.spill_choices
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
    pub const fn policy(self) -> RecoveryClassificationPolicy {
        self.policy
    }
    pub const fn usage(self) -> OptimizationWorkUsage {
        self.usage
    }
    pub const fn function_count(self) -> usize {
        self.function_count
    }
    pub const fn classification_count(self) -> usize {
        self.classification_count
    }
    pub const fn immediate_candidate_count(self) -> usize {
        self.immediate_candidate_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedRecoveryClassifications {
    pub(crate) plan: RecoveryClassificationPlan,
    pub(crate) receipt: RecoveryClassificationValidationReceipt,
}

impl ValidatedRecoveryClassifications {
    pub const fn plan(&self) -> &RecoveryClassificationPlan {
        &self.plan
    }
    pub const fn receipt(&self) -> RecoveryClassificationValidationReceipt {
        self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecoveryClassificationError {
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
    ChoiceMismatch {
        function: usize,
    },
    VictimMismatch {
        function: usize,
        register: u32,
    },
    ClassificationMismatch {
        function: usize,
    },
    UsageMismatch,
}

impl std::fmt::Display for RecoveryClassificationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "Terminal recovery classification failed: {self:?}"
        )
    }
}

impl std::error::Error for RecoveryClassificationError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryClassificationDecodeError {
    Truncated,
    WrongMagic,
    UnsupportedVersion(u32),
    UnknownPolicy(u8),
    UnknownOption(u8),
    UnknownVictimRole(u8),
    UnknownClassification(u8),
    UnknownReason(u8),
    UnknownScalarType(u8),
    UnknownIntegerValue(u8),
    UnknownOrigin(u8),
    UnknownDefinitionSite(u8),
    UnknownProvenance(u8),
    InvalidBudget,
    InvalidUsage,
    InvalidFuelSchedule(u32),
    InvalidMachineId(u64),
    InvalidBlockId(u64),
    InvalidOperationId(u64),
    InvalidValueId(u64),
    InvalidEdgeId(u64),
    InvalidObligationId(u64),
    InvalidIntegerType,
    LengthOverflow,
    IdentityMismatch,
    TrailingBytes,
}

impl std::fmt::Display for RecoveryClassificationDecodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid Terminal recovery-classification encoding: {self:?}"
        )
    }
}

impl std::error::Error for RecoveryClassificationDecodeError {}
