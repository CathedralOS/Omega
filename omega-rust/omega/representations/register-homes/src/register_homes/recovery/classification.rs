//! Unchecked recovery eligibility, policy, and canonical transport.

mod codec;
mod identity;
pub use identity::recovery_classification_identity;

use optimization_core::{OptimizationUnitIdentity, OptimizationWorkBudget, OptimizationWorkUsage};
use optimization_unit::ValueDefinitionSite;
use register_model::{RegisterClassId, RegisterViewId, TargetRegisterEnvironmentIdentity};
use selected_instructions::{
    SelectedBlockId, SelectedInstructionId, SelectedInstructionPlanIdentity,
    SelectedInstructionProvenance, VirtualRegisterId, VirtualRegisterOrigin,
};
use semantic_vocabulary::{FuelScheduleIdentity, IntegerValue, MachineId, ScalarType, ValueId};

use crate::{AllocationLegalityIdentity, AllocatorAvailabilityIdentity, SpillChoiceIdentity};

use selected_instructions::{LiveRangeIdentity, LiveRangePoint};

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
