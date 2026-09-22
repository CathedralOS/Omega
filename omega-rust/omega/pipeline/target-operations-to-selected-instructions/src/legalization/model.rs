use legalized_operations::{LegalizedOperationPlan, LegalizedOperationPlanIdentity};
use optimization_core::OptimizationValidatorIdentity;

pub(crate) fn legalization_validator_identity() -> OptimizationValidatorIdentity {
    OptimizationValidatorIdentity::from_canonical_bytes(
        b"omega.terminal-target-legalization-independent-replay.v53",
    )
}

/// The v22 validator identity, kept as a test control against the current one.
#[cfg(test)]
pub(crate) fn legalization_validator_identity_v22_legacy() -> OptimizationValidatorIdentity {
    OptimizationValidatorIdentity::from_canonical_bytes(
        b"omega.terminal-target-legalization-independent-replay.v22",
    )
}

/// The v21 validator identity, kept as a test control against the current one.
#[cfg(test)]
pub(crate) fn legalization_validator_identity_v21_legacy() -> OptimizationValidatorIdentity {
    OptimizationValidatorIdentity::from_canonical_bytes(
        b"omega.terminal-target-legalization-independent-replay.v21",
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
    /// The raw target, abstract, and optimization-unit custody disagree, or a
    /// node's own payload violates its representation. This names a producer
    /// or consumer defect, never a missing lowering.
    SourceCustodyMismatch,
    /// The node is well formed, but this stage has no legal scalar instruction
    /// for its operation family at its scalar type (for example signed
    /// saturating arithmetic, whose only legalized kinds are u64). This is an
    /// implementation limit rather than a custody disagreement, so the rejected
    /// operation is retained for the diagnostic instead of collapsing into
    /// `SourceCustodyMismatch`.
    UnsupportedScalarOperation {
        machine: semantic_vocabulary::MachineId,
        operation: abstract_operations::AbstractOperation,
    },
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
        match self {
            Self::UnsupportedScalarOperation { machine, operation } => write!(
                formatter,
                "target legalization has no legal scalar instruction for {operation:?} in machine {machine:?}"
            ),
            _ => write!(formatter, "target legalization failed: {self:?}"),
        }
    }
}

impl std::error::Error for LegalizationError {}
