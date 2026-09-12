use legalized_operations::{LegalizedOperationPlan, LegalizedOperationPlanIdentity};
use optimization_core::OptimizationValidatorIdentity;

pub fn legalization_validator_identity() -> OptimizationValidatorIdentity {
    OptimizationValidatorIdentity::from_canonical_bytes(
        b"omega.terminal-target-legalization-independent-replay.v50",
    )
}

#[doc(hidden)]
pub fn legalization_validator_identity_v22_legacy() -> OptimizationValidatorIdentity {
    OptimizationValidatorIdentity::from_canonical_bytes(
        b"omega.terminal-target-legalization-independent-replay.v22",
    )
}

#[doc(hidden)]
pub fn legalization_validator_identity_v21_legacy() -> OptimizationValidatorIdentity {
    OptimizationValidatorIdentity::from_canonical_bytes(
        b"omega.terminal-target-legalization-independent-replay.v21",
    )
}

#[doc(hidden)]
pub fn legalization_validator_identity_v20_legacy() -> OptimizationValidatorIdentity {
    OptimizationValidatorIdentity::from_canonical_bytes(
        b"omega.terminal-target-legalization-independent-replay.v20",
    )
}

#[doc(hidden)]
pub fn legalization_validator_identity_v19_legacy() -> OptimizationValidatorIdentity {
    OptimizationValidatorIdentity::from_canonical_bytes(
        b"omega.terminal-target-legalization-independent-replay.v19",
    )
}

#[doc(hidden)]
pub fn legalization_validator_identity_v18_legacy() -> OptimizationValidatorIdentity {
    OptimizationValidatorIdentity::from_canonical_bytes(
        b"omega.terminal-target-legalization-independent-replay.v18",
    )
}

#[doc(hidden)]
pub fn legalization_validator_identity_v17_legacy() -> OptimizationValidatorIdentity {
    OptimizationValidatorIdentity::from_canonical_bytes(
        b"omega.terminal-target-legalization-independent-replay.v17",
    )
}

/// Opaque custody of the canonical target-legal projection.
///
/// This carrier grants no instruction-selection, liveness, allocation,
/// emission, or publication authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedLegalizedOperations {
    pub(super) plan: LegalizedOperationPlan,
    pub(super) receipt: LegalizationValidationReceipt,
}

impl ValidatedLegalizedOperations {
    pub const fn plan(&self) -> &LegalizedOperationPlan {
        &self.plan
    }

    pub const fn receipt(&self) -> LegalizationValidationReceipt {
        self.receipt
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LegalizationValidationReceipt {
    pub(super) identity: LegalizedOperationPlanIdentity,
    pub(super) validator: OptimizationValidatorIdentity,
    pub(super) optimization_unit: optimization_core::OptimizationUnitIdentity,
    pub(super) fuel_schedule: semantic_vocabulary::FuelScheduleIdentity,
    pub(super) target: target::NativeTarget,
    pub(super) function_count: usize,
}

impl LegalizationValidationReceipt {
    pub const fn identity(self) -> LegalizedOperationPlanIdentity {
        self.identity
    }

    pub const fn validator(self) -> OptimizationValidatorIdentity {
        self.validator
    }

    pub const fn optimization_unit(self) -> optimization_core::OptimizationUnitIdentity {
        self.optimization_unit
    }

    pub const fn fuel_schedule(self) -> semantic_vocabulary::FuelScheduleIdentity {
        self.fuel_schedule
    }

    pub const fn target(self) -> target::NativeTarget {
        self.target
    }

    pub const fn function_count(self) -> usize {
        self.function_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LegalizationError {
    SourceCustodyMismatch,
    UnsupportedSourceShape {
        function: usize,
    },
    UnsupportedCondition {
        function: usize,
    },
    MissingConstantDefinition {
        function: usize,
        arm_edge: semantic_vocabulary::EdgeId,
    },
    MissingFuelProvenance {
        function: usize,
    },
    NonCanonicalLegalizedPlan,
}

impl std::fmt::Display for LegalizationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "target legalization failed: {self:?}")
    }
}

impl std::error::Error for LegalizationError {}
