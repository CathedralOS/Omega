/*
`optimization-core` defines the target-independent vocabulary, canonical wire
encodings, domain-separated identities, and publication records shared across
Omega's optimization pipeline.

We keep this crate free of executable optimizer passes so every compiler stage,
artifact manifest, and release tool can depend on one authoritative registry of
optimization names, phase assignments, rule contracts, and decision records
without pulling in IR representations or target lowerers.
*/

/*
The subsystem is organized around `optimization_core.rs`, which declares the
closed `Optimization` and `OptimizationExecutionPhase` vocabularies and their
canonical selection codec (`OMGOPT\0\0` v16):
- `contracts/`: rule declarations (`OptimizationRuleContract`), analysis bitsets
  (`AnalysisSet`, `AnalysisInvalidationSet`), work budgets
  (`OptimizationWorkBudget`), and candidate verdicts
  (`OptimizationCandidateVerdict`).
- `identities/`: domain-separated 32-byte SHA-256 identities carried between
  stages (`OptimizationRuleIdentity`, `OptimizationPassIdentity`,
  `OptimizationIdentityBundle`, and stage/artifact manifest identities).
- `manifest/`: canonical decision (`OMGDEC\0\0` v5) and pass (`OMGPAR\0\0` v1)
  publication records and consumed-fact references (`OptimizationFactReference`).
- `decisions/`: replayable baseline decision logs (`OMGBDL\0\0` v1) and the
  external decision wire schema (`OMGEXD\0\0` / `OMGEXP\0\0` v2).
- `rollback/`: subtractive exact-name release rollback overlays and receipts.
- Shared wire primitives (`codec_bytes`, `cursor`, `digest_encoder`,
  `error_display`, `record_views`, `tagged_vocabulary`, `variant_accessors`)
  reused by downstream optimization and package crates.
*/

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
