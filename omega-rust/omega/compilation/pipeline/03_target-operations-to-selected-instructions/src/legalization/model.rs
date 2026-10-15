use legalized_operations::{LegalizedOperationPlan, LegalizedOperationPlanIdentity};
use optimization_core::OptimizationValidatorIdentity;

pub(crate) fn legalization_validator_identity() -> OptimizationValidatorIdentity {
    OptimizationValidatorIdentity::from_canonical_bytes(
        b"omega.terminal-target-legalization-independent-replay.v55",
    )
}

/// Opaque custody of the canonical target-legal projection.
///
/// This carrier grants no instruction-selection, liveness, allocation,
/// emission, or publication authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ValidatedLegalizedOperations {
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
pub(crate) struct LegalizationValidationReceipt {
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LegalizationError {
    /// The raw target, abstract, and optimization-unit custody disagree, or a
    /// node's own payload violates its representation. This names a producer
    /// or consumer defect, never a missing lowering. `machine` and `operation`
    /// name the function being projected and the node reached when the refusal
    /// was raised, the same authored coordinates `UnsupportedScalarOperation`
    /// carries; whole-plan checks that precede any function leave both `None`.
    /// `site` is the check that refused (`LegalizationError::custody`), since
    /// some two hundred checks share this spelling and the variant alone cannot
    /// say which one fired; it is withheld from an author's diagnostic because
    /// only a developer can act on it.
    SourceCustodyMismatch {
        machine: Option<semantic_vocabulary::MachineId>,
        operation: Option<semantic_vocabulary::OperationId>,
        site: optimization_core::CompilerCheckSite,
    },
    /// The node is well formed, but this stage has no legal scalar instruction
    /// for its operation family at its scalar type (for example a port write
    /// or an abstract fused multiply-add, or integer arithmetic at a
    /// non-native width). This is an implementation limit rather than a
    /// custody disagreement, so the rejected operation is retained for the
    /// diagnostic instead of collapsing into `SourceCustodyMismatch`.
    UnsupportedScalarOperation {
        machine: semantic_vocabulary::MachineId,
        operation: abstract_operations::AbstractOperation,
    },
    UnsupportedSourceShape {
        function: usize,
    },
    ReferenceCompletion {
        reason: terminal_semantics::ReferenceCompletionError,
    },
    ReferenceResult {
        reason: &'static str,
    },
    /// Disposal selected storage that an independently reconstructed loan still
    /// protects. Preserve the selected home instead of requiring trace builds.
    ReferenceDisposalConflict {
        machine: semantic_vocabulary::MachineId,
        place: semantic_vocabulary::PlaceId,
    },
}

impl std::fmt::Display for LegalizationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ReferenceCompletion { reason } => formatter.write_str(reason.description()),
            Self::UnsupportedScalarOperation { machine, operation } => write!(
                formatter,
                "target legalization has no legal scalar instruction for {operation:?} in machine {machine:?}"
            ),
            _ => write!(formatter, "target legalization failed: {self:?}"),
        }
    }
}

impl std::error::Error for LegalizationError {}

impl LegalizationError {
    /// A custody disagreement, carrying the caller's check site for a
    /// developer and no authored coordinate yet: the per-function projection
    /// attaches the machine and the node it reached on the way out.
    #[track_caller]
    pub fn custody() -> Self {
        Self::SourceCustodyMismatch {
            machine: None,
            operation: None,
            site: optimization_core::CompilerCheckSite::caller(),
        }
    }

    /// Attach the function being projected and the node reached to a refusal
    /// raised beneath the per-function projection, which is the only place
    /// holding both coordinates. Other refusals already carry their own and
    /// pass through unchanged.
    pub(crate) fn attributed(
        self,
        machine: semantic_vocabulary::MachineId,
        operation: Option<semantic_vocabulary::OperationId>,
    ) -> Self {
        match self {
            Self::SourceCustodyMismatch {
                machine: None,
                operation: None,
                site,
            } => Self::SourceCustodyMismatch {
                machine: Some(machine),
                operation,
                site,
            },
            other => other,
        }
    }
}
