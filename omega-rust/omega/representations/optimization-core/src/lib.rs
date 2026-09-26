//! Optimizer module role: crate map. Stable, target-independent vocabulary for Omega optimization inputs.
//!
//! Start at `optimization_core.rs`: the authoritative exact-name registry and
//! canonical selection codec, leading into `contracts` (rule and budget
//! contracts), `identities` (the domain-separated identities carried between
//! stages), `manifest` (their common publication records) and `decisions`
//! (baseline logs and the external decision wire schema, not candidate
//! selection). This crate has no executable optimizer.

mod optimization_core;

pub use optimization_core::contracts::{
    AnalysisInvalidationSet, AnalysisKind, AnalysisSet, CoreContractDecodeError,
    InvalidOptimizationRuleContract, InvalidOptimizationWorkBudget, OptimizationCandidateVerdict,
    OptimizationReasonCode, OptimizationRuleContract, OptimizationSafetyClass,
    OptimizationWorkBudget,
};
pub use optimization_core::decisions::{
    BaselineDecisionLog, BaselineDecisionLogBuilder, BaselineDecisionLogDecodeError,
    BaselineDecisionOutcome, BaselineDecisionRecord, BaselineDecisionRecordError,
    ExternalCandidateFeatures, ExternalDecisionAction, ExternalDecisionContext,
    ExternalDecisionLog, ExternalDecisionPoint, ExternalDecisionSchemaError,
    ValidatedCandidateSummary, external_psi_decision_schema_v2_identity,
    psi_target_neutral_decision_target_v2_identity,
};
pub use optimization_core::identities::{
    AcceptedObligationFactIdentity, CanonicalIdentityEncoder, DuplicateOptimizationRuleIdentity,
    FunctionFragmentEmissionIdentity, FunctionFragmentEmissionManifestIdentity,
    FunctionFragmentObjectContainerManifestIdentity, FunctionFragmentTextSectionManifestIdentity,
    FunctionRelativeOptimizationRealizationManifestIdentity, IdentityBundleDecodeError,
    IdentityDecodeError, NativeOptimizationProjectionIdentity, OptimizationCandidateIdentity,
    OptimizationDecisionIdentity, OptimizationDecisionLogIdentity,
    OptimizationDecisionSchemaIdentity, OptimizationDecisionTargetIdentity,
    OptimizationIdentityBundle, OptimizationIdentityBundleIdentity, OptimizationPassIdentity,
    OptimizationRuleIdentity, OptimizationRuleSetIdentity, OptimizationUnitIdentity,
    OptimizationValidatorIdentity, OptimizationWorkloadProfileIdentity,
    OptimizedAbstractPlanProjectionIdentity, OptimizedBoundaryOccurrenceIdentity,
    OptimizedObjectArtifactIdentity, OptimizedObjectArtifactManifestIdentity,
    OptimizedOperatorOccurrenceIdentity, OptimizedOrdinaryCallableEntryManifestIdentity,
    OptimizedProgramStorageSemanticWrapperObjectContainerIdentity,
    OptimizedProgramStorageSemanticWrapperObjectIdentity,
    OptimizedProgramStorageSemanticWrapperObjectManifestIdentity,
    OptimizedTerminalOrdinaryCallableEntryIdentity, OwnershipFrontierFactIdentity,
    PostAllocationOptimizationManifestIdentity, PreAllocationOptimizationCompletionIdentity,
    PrePhysicalOptimizationManifestIdentity, ProofQuestionIdentity,
    RelocationFreeObjectContainerIdentity, RelocationFreeObjectPlanIdentity,
    ScalarConstantFactIdentity, SelectedLoweringOptimizationCompletionIdentity,
    TargetCostModelIdentity, TerminalRelocationFreeTextSectionIdentity,
    TransformationLedgerIdentity, ValueRangeFactIdentity,
};
pub use optimization_core::manifest::{
    InvalidOptimizationManifestRecord, OptimizationDecisionRecord, OptimizationFactReference,
    OptimizationFactReferenceDecodeError, OptimizationManifestDecodeError,
    OptimizationPassManifestRecord, OptimizationWorkUsage,
};
#[cfg(any(test, feature = "test-support"))]
pub use optimization_core::mutation_matrix::{
    MutationOutcome, OneFieldSubstitutionMatrix, custody_field_inventory,
    run_one_field_substitution_matrix,
};
pub use optimization_core::report_request::OptimizationReportRequest;
pub use optimization_core::{
    DuplicateOptimization, Optimization, OptimizationCatalogDescriptor, OptimizationExecutionPhase,
    OptimizationPhaseMismatch, OptimizationPhaseSelections, OptimizationSelectionIdentity,
    OptimizationSelections, PostTerminalOptimizationSelectionProjection,
    PostTerminalOptimizationSelections, PreTerminalOptimizationSelection,
    PsiOptimizationSelectionProjection, SelectionDecodeError,
};
