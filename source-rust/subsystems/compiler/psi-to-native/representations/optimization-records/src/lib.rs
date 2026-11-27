//! Rule contracts, work counters and decision evidence carried through native optimization.
#![forbid(unsafe_code)]

mod contracts;
mod decisions;
mod identities;
mod manifest;
mod record_encoding;

#[cfg(test)]
mod canonical_rows;

pub(crate) use contracts::CoreContractDecodeError;
pub use contracts::{
    AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OPTIMIZATION_REFERENCE_EXTENT,
    OptimizationCandidateVerdict, OptimizationReasonCode, OptimizationRuleContract,
    OptimizationSafetyClass, OptimizationWorkBudget,
};
pub use decisions::{
    BaselineDecisionLog, BaselineDecisionLogBuilder, BaselineDecisionLogDecodeError,
    BaselineDecisionOutcome, BaselineDecisionRecordError, ExternalCandidateFeatures,
    ExternalDecisionAction, ExternalDecisionContext, ExternalDecisionLog, ExternalDecisionPoint,
    ExternalDecisionSchemaError, ValidatedCandidateSummary, external_psi_decision_schema_identity,
    psi_target_neutral_decision_target_identity,
};
pub use identities::{
    AcceptedObligationFactIdentity, CanonicalIdentityEncoder, NativeOptimizationProjectionIdentity,
    OptimizationCandidateIdentity, OptimizationDecisionIdentity, OptimizationDecisionLogIdentity,
    OptimizationDecisionSchemaIdentity, OptimizationDecisionTargetIdentity,
    OptimizationIdentityBundle, OptimizationIdentityBundleIdentity, OptimizationPassIdentity,
    OptimizationRuleIdentity, OptimizationRuleSetIdentity, OptimizationUnitIdentity,
    OptimizationValidatorIdentity, OptimizationWorkloadProfileIdentity,
    OptimizedAbstractPlanProjectionIdentity, OptimizedBoundaryOccurrenceIdentity,
    OptimizedOperatorOccurrenceIdentity, OwnershipFrontierFactIdentity,
    PostAllocationOptimizationManifestIdentity, PreAllocationOptimizationCompletionIdentity,
    PrePhysicalOptimizationManifestIdentity, ScalarConstantFactIdentity,
    SelectedLoweringOptimizationCompletionIdentity, TargetCostModelIdentity,
    TransformationLedgerIdentity, ValueRangeFactIdentity,
};
pub(crate) use manifest::OptimizationFactReferenceDecodeError;
pub use manifest::{
    InvalidOptimizationManifestRecord, OptimizationDecisionRecord, OptimizationFactReference,
    OptimizationPassManifestRecord, OptimizationWorkUsage,
};
pub use optimization_selections::{
    Optimization, OptimizationCatalogDescriptor, OptimizationExecutionPhase,
    OptimizationPhaseMismatch, OptimizationPhaseSelections, OptimizationSelectionIdentity,
    OptimizationSelections, PostTerminalOptimizationSelectionProjection,
    PostTerminalOptimizationSelections, PsiOptimizationSelectionProjection,
};
