use omega_legalized_operations::{LegalizedOperationPlan, LegalizedOperationPlanIdentity};
use omega_optimization_core::OptimizationValidatorIdentity;

pub fn legalization_validator_identity() -> OptimizationValidatorIdentity {
    OptimizationValidatorIdentity::from_canonical_bytes(
        b"omega.terminal-target-legalization-independent-replay.v9",
    )
}

/// Opaque custody of the canonical V9 target-legal projection.
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
    pub(super) optimization_unit: omega_optimization_core::OptimizationUnitIdentity,
    pub(super) fuel_schedule: psi_core::FuelScheduleIdentity,
    pub(super) target: omega_target::NativeTarget,
    pub(super) function_count: usize,
    pub(super) decomposition_count: usize,
}

impl LegalizationValidationReceipt {
    pub const fn identity(self) -> LegalizedOperationPlanIdentity {
        self.identity
    }

    pub const fn validator(self) -> OptimizationValidatorIdentity {
        self.validator
    }

    pub const fn optimization_unit(self) -> omega_optimization_core::OptimizationUnitIdentity {
        self.optimization_unit
    }

    pub const fn fuel_schedule(self) -> psi_core::FuelScheduleIdentity {
        self.fuel_schedule
    }

    pub const fn target(self) -> omega_target::NativeTarget {
        self.target
    }

    pub const fn function_count(self) -> usize {
        self.function_count
    }

    /// Independently replayed non-identity legalization occurrence groups.
    pub const fn decomposition_count(self) -> usize {
        self.decomposition_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LegalizationError {
    RankedCountdownNotYetSelectable {
        machine: psi_core::MachineId,
    },
    SourceCustodyMismatch,
    UnsupportedSourceShape {
        function: usize,
    },
    UnsupportedIntegerShape {
        function: usize,
    },
    UnsupportedCondition {
        function: usize,
    },
    MissingConstantDefinition {
        function: usize,
        arm_edge: psi_core::EdgeId,
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
