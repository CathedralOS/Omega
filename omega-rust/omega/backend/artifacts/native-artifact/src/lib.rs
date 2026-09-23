#![forbid(unsafe_code)]

//! Authority-free canonical Terminal-Psi to native artifact handoff.
//!
//! This crate deliberately has no dependency on source, syntax, typed,
//! checked, or source-derived provider-plan carriers.
//! It owns only canonical Terminal bytes, target artifacts, and the exact
//! source-free identity projections needed to replay their joins.
//!
//! Start at `native_artifact.rs` for artifact construction and replay,
//! `callable_entry.rs` for validated callable-entry staging, or
//! `semantic_wrapper_object.rs` for the optimized ProgramStorage
//! semantic-wrapper object records and codec that native realization stages.

mod callable_entry;
mod native_artifact;
mod physical;
mod semantic_wrapper_object;

pub use callable_entry::{
    OptimizedOrdinaryCallableEntryCustodyReceipt, OptimizedOrdinaryCallableEntryDecodeError,
    OptimizedOrdinaryCallableEntryDisposition, OptimizedOrdinaryCallableEntryError,
    OptimizedOrdinaryCallableEntryManifest, OptimizedOrdinaryCallableEntryManifestDecodeError,
    OptimizedOrdinaryCallableEntryRecord, OptimizedOrdinaryCallableEntryStage,
    OptimizedOrdinaryCallableEntryUnavailableData, OptimizedOrdinaryCallableParameter,
    OptimizedOrdinaryCallableResult, OptimizedOrdinaryCallableReturn,
    StagedValidatedOptimizedOrdinaryCallableEntry, ValidatedOptimizedOrdinaryCallableEntryManifest,
    stage_validated_optimized_ordinary_callable_entry, validate_optimized_ordinary_callable_entry,
};
pub use image_emission::BoundaryExecutionRecord;
pub use native_artifact::{
    DynamicElfNativeArtifact, DynamicElfNativeArtifactEmissionParts,
    DynamicElfNativeArtifactIdentity, DynamicElfNativeArtifactParts, NativeArtifact,
    NativeArtifactEmissionParts, NativeArtifactIdentity, NativeArtifactParts,
    NativePhysicalEvidenceScope, NativeProviderExecution, NativeSelectedProviderClosureDigest,
    NativeSelectedProviderPlan, NativeSelectedProviderPlanDigest,
};
pub use physical::{
    BoundaryTraitSettlement, BoundaryTraitSettlementParts, BoundaryTraitSettlementRole,
    CompilerBuiltinResult, CompilerBuiltinScalarArgument, DynamicCallDispatch,
    DynamicCallDispatchParts, DynamicCallRelocationCustody, NativeByteSpan,
    NativeCompilerBuiltinCatalogIdentity, NativeOptimizationProjection, NativePhysicalChild,
    NativePhysicalChildParts, NativePhysicalEvidence, NativePhysicalEvidenceGap,
    NativePhysicalEvidenceGapSubject, NativePhysicalEvidenceParts, NativePhysicalOccurrence,
    NormalizedForeignCallImportField, NormalizedForeignCallRelocation,
    NormalizedForeignCallbackRelocation, NormalizedForeignCallbackRelocations,
    OptimizedBoundaryOccurrence, OptimizedOperatorOccurrence, PhysicalChildParent,
    PhysicalRelocationDisposition, ValidatedOptimizedNativePhysicalEvidenceScope,
};
pub use semantic_wrapper_object::{
    OptimizedProgramStorageSemanticWrapperCallResolution,
    OptimizedProgramStorageSemanticWrapperCallResolutionState,
    OptimizedProgramStorageSemanticWrapperObjectContainer,
    OptimizedProgramStorageSemanticWrapperObjectCustodyReceipt,
    OptimizedProgramStorageSemanticWrapperObjectDecodeError,
    OptimizedProgramStorageSemanticWrapperObjectManifest,
    OptimizedProgramStorageSemanticWrapperObjectPlan,
    OptimizedProgramStorageSemanticWrapperObjectRecordError,
    OptimizedProgramStorageSemanticWrapperObjectStage,
    OptimizedProgramStorageSemanticWrapperObjectSymbol,
    OptimizedProgramStorageSemanticWrapperObjectSymbolRole,
    OptimizedProgramStorageSemanticWrapperObjectUnavailableData,
    ValidatedOptimizedProgramStorageSemanticWrapperObjectManifest,
    compose_optimized_program_storage_semantic_wrapper_object,
    decode_optimized_program_storage_semantic_wrapper_object,
    encode_optimized_program_storage_semantic_wrapper_object,
    encode_optimized_program_storage_semantic_wrapper_object_preserving_seal,
};
