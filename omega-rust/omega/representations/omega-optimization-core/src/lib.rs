//! Optimizer module role: crate map. Stable, target-independent vocabulary for Omega optimization inputs.
//!
//! `selection` is the authoritative exact-name registry and canonical selection
//! codec. `contracts` defines rule and budget contracts. `identities` owns the
//! domain-separated identities carried between stages, while `manifest` owns
//! their common publication records. `decisions` owns baseline logs and the
//! external decision wire schema, not candidate selection. This crate has no
//! executable optimizer.

mod contracts;
mod decisions;
pub use decisions::*;
mod report_request;
pub use report_request::OptimizationReportRequest;
mod identities;
mod manifest;
mod selection;

pub use contracts::{
    AnalysisInvalidationSet, AnalysisKind, AnalysisSet, CoreContractDecodeError,
    InvalidOptimizationRuleContract, InvalidOptimizationWorkBudget, OptimizationCandidateVerdict,
    OptimizationReasonCode, OptimizationRuleContract, OptimizationSafetyClass,
    OptimizationWorkBudget,
};
pub use identities::{
    AcceptedObligationFactIdentity, DuplicateOptimizationRuleIdentity,
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
    PostAllocationOptimizationManifestIdentity, PrePhysicalOptimizationManifestIdentity,
    ProofQuestionIdentity, RelocationFreeObjectContainerIdentity, RelocationFreeObjectPlanIdentity,
    ScalarConstantFactIdentity, SelectedLoweringOptimizationCompletionIdentity,
    TargetCostModelIdentity, TerminalRelocationFreeTextSectionIdentity,
    TransformationLedgerIdentity, ValueRangeFactIdentity,
};
pub use manifest::{
    InvalidOptimizationManifestRecord, OptimizationDecisionRecord, OptimizationFactReference,
    OptimizationFactReferenceDecodeError, OptimizationManifestDecodeError,
    OptimizationPassManifestRecord, OptimizationWorkUsage,
};
pub use selection::{
    DuplicateOptimization, Optimization, OptimizationCatalogDescriptor, OptimizationExecutionPhase,
    OptimizationPhaseMismatch, OptimizationPhaseSelections, OptimizationSelectionIdentity,
    OptimizationSelections, PostTerminalOptimizationSelectionProjection,
    PostTerminalOptimizationSelections, PreTerminalOptimizationSelection,
    PsiOptimizationSelectionProjection, SelectionDecodeError,
};
