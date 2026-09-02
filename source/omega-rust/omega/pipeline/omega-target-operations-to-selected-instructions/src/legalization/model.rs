use omega_legalized_operations::{LegalizedOperationPlan, LegalizedOperationPlanIdentity};
use omega_optimization_core::OptimizationValidatorIdentity;

pub fn legalization_validator_identity() -> OptimizationValidatorIdentity {
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
pub struct ProjectedStructuralCallReturnLegalizationReceipt {
    pub(super) caller: psi_core::MachineId,
    pub(super) callee: psi_core::MachineId,
    pub(super) projected_qualification_count: usize,
}

impl ProjectedStructuralCallReturnLegalizationReceipt {
    pub const fn caller(self) -> psi_core::MachineId {
        self.caller
    }

    pub const fn callee(self) -> psi_core::MachineId {
        self.callee
    }

    pub const fn projected_qualification_count(self) -> usize {
        self.projected_qualification_count
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
    pub(super) projected_structural_call_return:
        Option<ProjectedStructuralCallReturnLegalizationReceipt>,
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

    pub const fn projected_structural_call_return(
        self,
    ) -> Option<ProjectedStructuralCallReturnLegalizationReceipt> {
        self.projected_structural_call_return
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectedStructuralCallReturnLegalizationError {
    UnsupportedSourceShape,
    UnsupportedTargetShape,
    SourceTargetMismatch,
    UnexpectedProposedClosure,
    NonCanonicalProposedClosure,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LegalizationError {
    RankedCountdownNotYetSelectable {
        machine: psi_core::MachineId,
    },
    AttachedUnitStructuralScalarNotYetSelectable {
        machine: psi_core::MachineId,
        operation: psi_core::OperationId,
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
    ProjectedStructuralCallReturn(ProjectedStructuralCallReturnLegalizationError),
}

impl From<ProjectedStructuralCallReturnLegalizationError> for LegalizationError {
    fn from(error: ProjectedStructuralCallReturnLegalizationError) -> Self {
        Self::ProjectedStructuralCallReturn(error)
    }
}

impl std::fmt::Display for LegalizationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "target legalization failed: {self:?}")
    }
}

impl std::error::Error for LegalizationError {}
