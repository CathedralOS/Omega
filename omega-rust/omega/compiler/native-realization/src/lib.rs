#![forbid(unsafe_code)]

//! Optimizer module role: crate map. Native product coordination over the program stages.
//!
//! This compiler owner accepts Terminal Psi and a separate realization request,
//! settles entry/provider custody, and sequences transforms through native
//! publication. It is not another program stage: the Omega program route
//! begins at `terminal-psi-to-abstract-operations`. Target setup is supplied by
//! backend owners. Component policy and installation remain outside this owner.

mod entry_settlement;
mod native_pipeline;
mod native_realization;
mod optimized_semantic_wrapper_encoding;
mod optimized_semantic_wrapper_object;

pub use abstract_operations_to_target_operations::AdmittedIeeeFloatFmaSettlement;
pub use entry_settlement::{
    NativeProgramEntrySettlement, NativeProgramEntrySettlementError,
    ValidatedNativeProgramEntrySettlement, validate_native_program_entry_settlement,
};
pub use image_emission::ExecutableImageEmissionRequest;
pub use native_artifact::{
    BoundaryExecutionRecord, BoundaryTraitSettlement, BoundaryTraitSettlementParts,
    BoundaryTraitSettlementRole, DynamicElfNativeArtifact, DynamicElfNativeArtifactParts,
    NativeArtifact, NativeArtifactParts, NativeByteSpan, NativePhysicalChild,
    NativePhysicalChildParts, NativePhysicalEvidence, NativePhysicalEvidenceParts,
    NativePhysicalEvidenceScope, NativePhysicalOccurrence, NativeProviderExecution,
    NativeSelectedProviderPlan, NativeSelectedProviderPlanDigest, NormalizedForeignCallRelocation,
    NormalizedForeignCallbackRelocation, NormalizedForeignCallbackRelocations, PhysicalChildParent,
    PhysicalRelocationDisposition,
};
#[cfg(any(test, feature = "test-support"))]
pub use native_pipeline::stage_optimized_verified_physical_pipeline_with_provider_executions;
pub use native_pipeline::{
    EmptyOptimizationSelections, ExplicitOptimizationRequest, OptimizationPipelineError,
    OptimizationPipelineReport, OptimizationPipelineRequest,
    OptimizedVerifiedPhysicalPipelineError, StagedOptimizedVerifiedPhysicalPipeline,
    compiler_baseline_request_v1, optimization_pipeline_report,
    optimization_pipeline_report_from_object_artifact,
    optimization_pipeline_report_from_ordinary_callable_entry, optimize_artifact_sections,
    optimize_verified_abstract_input, stage_optimized_verified_physical_pipeline,
};
pub use native_realization::terminal_authority_permissions::{
    validate_package_terminal_authority_permissions,
    validate_retained_package_terminal_authority_permissions,
};
pub use native_realization::{
    COMPILER_INTRINSIC_TERMINAL_AUTHORITY_POLICY_VERSION, CallbackCustodyNativeRealizationError,
    CompilerIntrinsicTerminalAuthorityPolicy, MissingTerminalAuthorityPermission,
    NativeBoundaryRealization, NativeCallbackThunkSettlement, NativeCompilerBuiltinSettlement,
    NativeProviderSettlement, NativeRealizationRequest, PreparedNativeRealizationInput,
    RealizedNativeArtifactWithCallbackCustody, RequestedNativeArtifact,
    RequestedNativeArtifactError, SettledNativeArtifact,
    TERMINAL_AUTHORITY_PERMISSION_POLICY_VERSION, TERMINAL_AUTHORITY_POLICY_VERSION,
    TerminalAuthorityPermissionPolicy, TerminalAuthorityPermissionPolicyBuildError,
    TerminalAuthorityPermissionPolicyRow, TerminalAuthorityPolicy,
    TerminalAuthorityPolicyBuildError, TerminalAuthorityPolicyRow,
    UnclassifiedCompilerIntrinsicTerminalMechanism, UnclassifiedTerminalMechanism,
    conservative_syscall_terminal_mechanism, current_compiler_intrinsic_terminal_authority_policy,
    current_terminal_authority_permission_policy, current_terminal_authority_policy,
    normalized_foreign_terminal_mechanism,
    normalized_foreign_terminal_mechanism_with_callback_materializations,
    prepare_native_realization_input, realize_native_artifact,
    realize_native_artifact_with_callback_custody, realize_program_entry_native_artifact,
    terminal_authority_permission_policy_with_rows, terminal_authority_policy_with_rows,
};
pub use optimized_semantic_wrapper_encoding::{
    OptimizedProgramStorageSemanticWrapperEncodingError,
    StagedOptimizedProgramStorageSemanticWrapperEncoding,
    select_optimized_program_storage_semantic_wrapper_encoding,
    validate_optimized_program_storage_semantic_wrapper_encoding,
};
pub use optimized_semantic_wrapper_object::{
    InstalledProgramStorageContinuationEvidenceError,
    OptimizedProgramStorageSemanticWrapperCallResolution,
    OptimizedProgramStorageSemanticWrapperCallResolutionState,
    OptimizedProgramStorageSemanticWrapperObjectContainer,
    OptimizedProgramStorageSemanticWrapperObjectCustodyReceipt,
    OptimizedProgramStorageSemanticWrapperObjectDecodeError,
    OptimizedProgramStorageSemanticWrapperObjectError,
    OptimizedProgramStorageSemanticWrapperObjectManifest,
    OptimizedProgramStorageSemanticWrapperObjectPlan,
    OptimizedProgramStorageSemanticWrapperObjectStage,
    OptimizedProgramStorageSemanticWrapperObjectSymbol,
    OptimizedProgramStorageSemanticWrapperObjectSymbolRole,
    OptimizedProgramStorageSemanticWrapperObjectUnavailableData,
    StagedValidatedOptimizedProgramStorageSemanticWrapperObject,
    ValidatedOptimizedProgramStorageSemanticWrapperObjectManifest,
    decode_optimized_program_storage_semantic_wrapper_object,
    encode_optimized_program_storage_semantic_wrapper_object,
    stage_validated_optimized_program_storage_semantic_wrapper_object,
    validate_installed_program_storage_continuation_evidence,
    validate_optimized_program_storage_semantic_wrapper_object,
};

#[cfg(test)]
mod tests;
