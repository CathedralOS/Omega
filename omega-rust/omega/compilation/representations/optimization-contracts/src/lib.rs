//! Native optimization selections, identities, rule contracts and carried decision records.
//! No compiler driver, rewrite, release request or source authority is retained here.
#![forbid(unsafe_code)]

mod optimization_core;

pub(crate) use optimization_core::contracts::CoreContractDecodeError;
pub use optimization_core::contracts::{
    AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OPTIMIZATION_REFERENCE_EXTENT,
    OptimizationCandidateVerdict, OptimizationReasonCode, OptimizationRuleContract,
    OptimizationSafetyClass, OptimizationWorkBudget,
};
pub use optimization_core::decisions::{
    BaselineDecisionLog, BaselineDecisionLogBuilder, BaselineDecisionLogDecodeError,
    BaselineDecisionOutcome, BaselineDecisionRecordError, ExternalCandidateFeatures,
    ExternalDecisionAction, ExternalDecisionContext, ExternalDecisionLog, ExternalDecisionPoint,
    ExternalDecisionSchemaError, ValidatedCandidateSummary, external_psi_decision_schema_identity,
    psi_target_neutral_decision_target_identity,
};
pub use optimization_core::identities::{
    AcceptedObligationFactIdentity, CanonicalIdentityEncoder, FunctionFragmentEmissionIdentity,
    FunctionFragmentEmissionManifestIdentity, FunctionFragmentObjectContainerManifestIdentity,
    FunctionFragmentTextSectionManifestIdentity,
    FunctionRelativeOptimizationRealizationManifestIdentity, NativeOptimizationProjectionIdentity,
    OptimizationCandidateIdentity, OptimizationDecisionIdentity, OptimizationDecisionLogIdentity,
    OptimizationDecisionSchemaIdentity, OptimizationDecisionTargetIdentity,
    OptimizationIdentityBundle, OptimizationIdentityBundleIdentity, OptimizationPassIdentity,
    OptimizationRuleIdentity, OptimizationRuleSetIdentity, OptimizationUnitIdentity,
    OptimizationValidatorIdentity, OptimizationWorkloadProfileIdentity,
    OptimizedAbstractPlanProjectionIdentity, OptimizedBoundaryOccurrenceIdentity,
    OptimizedObjectArtifactIdentity, OptimizedObjectArtifactManifestIdentity,
    OptimizedOperatorOccurrenceIdentity, OptimizedOrdinaryCallableEntryManifestIdentity,
    OptimizedTerminalOrdinaryCallableEntryIdentity, OwnershipFrontierFactIdentity,
    PostAllocationOptimizationManifestIdentity, PreAllocationOptimizationCompletionIdentity,
    PrePhysicalOptimizationManifestIdentity, RelocationFreeObjectContainerIdentity,
    RelocationFreeObjectPlanIdentity, ScalarConstantFactIdentity,
    SelectedLoweringOptimizationCompletionIdentity, TargetCostModelIdentity,
    TerminalRelocationFreeTextSectionIdentity, TransformationLedgerIdentity,
    ValueRangeFactIdentity,
};
pub(crate) use optimization_core::manifest::OptimizationFactReferenceDecodeError;
pub use optimization_core::manifest::{
    InvalidOptimizationManifestRecord, OptimizationDecisionRecord, OptimizationFactReference,
    OptimizationPassManifestRecord, OptimizationWorkUsage,
};
pub use optimization_core::{
    Optimization, OptimizationCatalogDescriptor, OptimizationExecutionPhase,
    OptimizationPhaseMismatch, OptimizationPhaseSelections, OptimizationSelectionIdentity,
    OptimizationSelections, PostTerminalOptimizationSelectionProjection,
    PostTerminalOptimizationSelections, PsiOptimizationSelectionProjection,
};
