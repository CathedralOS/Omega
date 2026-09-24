//! Typed rejection vocabulary for the projection boundary.

use crate::RuleRegistryError;
use crate::validation::{
    OptimizedAbstractPlanProjectionError, PrePhysicalOptimizationManifestError,
};
use optimization_core::OptimizationCandidateIdentity;
use optimization_unit_semantics::OptimizationUnitValidationError;
use semantic_vocabulary::MachineId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppliedDecisionCustodyAxis {
    ManifestRoster,
    ManifestPass,
    ManifestRuleSet,
    ManifestRuleOrder,
    RuleContract,
    RequiredAnalyses,
    InvalidatedAnalyses,
    SafetyClass,
    CommitPredictedCostDelta,
    AppliedRoster,
    ValidatedRoster,
    ValidatedPass,
    InputRevision,
    Verdict,
    CommitRoster,
    CommitDeclaration,
    Input,
    Candidate,
    Rule,
    Validator,
    ConsumedAnalyses,
    ConsumedFacts,
    BaselineRoster,
    BaselineInput,
    BaselineOutcome,
    BaselineRule,
    PredictedCostDelta,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedAbstractProjectionError {
    Registry(RuleRegistryError),
    FunctionRosterMismatch,
    InvalidFunctionParameter {
        machine: MachineId,
        position: usize,
    },
    InvalidBlockParameter {
        machine: MachineId,
        position: usize,
    },
    OperationOffsetOverflow(MachineId),
    InitialUnitProjection,
    CandidateReplay(OptimizationUnitValidationError),
    CommitReplayMismatch,
    FinalUnitReplayMismatch,
    AppliedDecisionCustody {
        candidate: Option<OptimizationCandidateIdentity>,
        axis: AppliedDecisionCustodyAxis,
    },
    LedgerCommitMismatch,
    ManifestUsageMismatch,
    ExternalDecisionRecordingMismatch,
    PsiSelectionProjectionMismatch,
    IndependentValidation(OptimizedAbstractPlanProjectionError),
    PrePhysicalManifest(PrePhysicalOptimizationManifestError),
}

impl std::fmt::Display for OptimizedAbstractProjectionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "cannot project optimized abstract plan: {self:?}"
        )
    }
}

impl std::error::Error for OptimizedAbstractProjectionError {}

pub(crate) const fn custody_error(
    candidate: Option<OptimizationCandidateIdentity>,
    axis: AppliedDecisionCustodyAxis,
) -> OptimizedAbstractProjectionError {
    OptimizedAbstractProjectionError::AppliedDecisionCustody { candidate, axis }
}
