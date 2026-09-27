#![forbid(unsafe_code)]

//! Authority-free canonical Terminal-Psi to native artifact handoff.
//!
//! This crate deliberately has no dependency on source, syntax, typed,
//! checked, or source-derived provider-plan carriers.
//! It owns only canonical Terminal bytes, target artifacts, and the exact
//! source-free identity projections needed to replay their joins.
//!
//! Start at `fragment_object_publication.rs` for fragment emission, entry
//! binding and object publication evidence,
//! `entry_receiver.rs` for executable receiver admission before construction,
//! `native_artifact.rs` for artifact construction and replay,
//! `proof.rs` for standalone native proof construction and verification,
//! `callable_entry.rs` for validated callable-entry staging, or
//! `semantic_wrapper_object.rs` for the optimized ProgramStorage
//! semantic-wrapper object records and codec that native realization stages.

mod callable_entry;
mod entry_receiver;
mod fragment_object_publication;
mod native_artifact;
pub mod optimized_semantic_wrapper_object;
mod physical;
mod proof;
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
pub use entry_receiver::admit_executable_entry_receiver;
pub use fragment_object_publication::{
    FragmentObjectPublicationRequest, PublishedFragmentObject, publish_fragment_object,
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

pub use proof::{
    NATIVE_PLACED_IMAGE_COVERAGE_GUARANTEE, NativeEvidenceError, NativePlacedImageEvidence,
    build_native_proof_sidecar, verify_native_proof_sidecar,
};
