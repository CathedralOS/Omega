//! Optimizer module role: crate map. Stable, target-independent vocabulary for Omega optimization inputs.
//!
//! Start at `optimization_core.rs`: the authoritative exact-name registry and
//! canonical selection codec, leading into `contracts` (rule and budget
//! contracts), `identities` (the domain-separated identities carried between
//! stages), `manifest` (their common publication records) and `decisions`
//! (baseline logs and the external decision wire schema, not candidate
//! selection). This crate has no executable optimizer.
//! `rollback` owns exact-name disabling, selection settlement and its receipt;
//! callers decide which execution phases their requested product permits.

pub mod codec_bytes;
pub mod cursor;
pub mod digest_encoder;
pub mod error_display;
mod optimization_core;
pub mod record_views;
pub mod tagged_vocabulary;
pub mod variant_accessors;

pub(crate) use optimization_core::DuplicateOptimization;
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
    ExternalDecisionSchemaError, ValidatedCandidateSummary,
    external_psi_decision_schema_v2_identity, psi_target_neutral_decision_target_v2_identity,
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
pub(crate) use optimization_core::manifest::OptimizationFactReferenceDecodeError;
pub use optimization_core::manifest::{
    InvalidOptimizationManifestRecord, OptimizationDecisionRecord, OptimizationFactReference,
    OptimizationPassManifestRecord, OptimizationWorkUsage,
};
#[cfg(any(test, feature = "test-support"))]
pub use optimization_core::mutation_matrix::{
    MutationOutcome, OneFieldSubstitutionMatrix, custody_field_inventory,
    run_one_field_substitution_matrix,
};
pub use optimization_core::refusal_site::CompilerCheckSite;
pub use optimization_core::report_request::OptimizationReportRequest;
pub use optimization_core::rollback::{
    OptimizationRollback, OptimizationRollbackInputError, OptimizationRollbackReceipt,
    OptimizationRollbackSettlement,
};
pub use optimization_core::{
    Optimization, OptimizationCatalogDescriptor, OptimizationExecutionPhase,
    OptimizationPhaseMismatch, OptimizationPhaseSelections, OptimizationSelectionIdentity,
    OptimizationSelections, PostTerminalOptimizationSelectionProjection,
    PostTerminalOptimizationSelections, PsiOptimizationSelectionProjection, encode_option_u64, hex,
};
