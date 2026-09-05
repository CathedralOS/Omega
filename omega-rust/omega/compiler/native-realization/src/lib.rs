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
pub use native_pipeline::*;
mod optimized_semantic_wrapper_encoding;
mod optimized_semantic_wrapper_object;
mod realization;

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
pub use optimized_semantic_wrapper_encoding::{
    OptimizedProgramStorageSemanticWrapperEncodingError,
    StagedOptimizedProgramStorageSemanticWrapperEncoding,
    select_optimized_program_storage_semantic_wrapper_encoding,
    validate_optimized_program_storage_semantic_wrapper_encoding,
};
pub use optimized_semantic_wrapper_object::*;
pub use realization::{
    COMPILER_INTRINSIC_TERMINAL_AUTHORITY_POLICY_VERSION, CallbackCustodyNativeRealizationError,
    CompilerIntrinsicTerminalAuthorityPolicy, MissingTerminalAuthorityPermission,
    NativeBoundaryRealization, NativeCallbackThunkSettlement, NativeCompilerBuiltinSettlement,
    NativeProviderSettlement, NativeRealizationRequest, PreparedNativeRealizationInput,
    RealizedNativeArtifactWithCallbackCustody, RequestedNativeArtifact,
    RequestedNativeArtifactError, RequestedNativeRealizationRequest, SettledNativeArtifact,
    TERMINAL_AUTHORITY_PERMISSION_POLICY_VERSION, TERMINAL_AUTHORITY_POLICY_VERSION,
    TerminalAuthorityPermissionPolicy, TerminalAuthorityPermissionPolicyBuildError,
    TerminalAuthorityPermissionPolicyRow, TerminalAuthorityPolicy,
    TerminalAuthorityPolicyBuildError, TerminalAuthorityPolicyRow,
    UnclassifiedCompilerIntrinsicTerminalMechanism, UnclassifiedTerminalMechanism,
    current_compiler_intrinsic_terminal_authority_policy,
    current_terminal_authority_permission_policy, current_terminal_authority_policy,
    normalized_foreign_terminal_mechanism,
    normalized_foreign_terminal_mechanism_with_callback_materializations,
    prepare_native_realization_input, realize_native_artifact,
    realize_native_artifact_with_callback_custody,
    realize_native_artifact_with_checked_boundary_operator_scope,
    realize_native_artifact_with_checked_boundary_operator_scope_and_prepared_input,
    realize_program_entry_native_artifact, realize_requested_native_artifact,
    realize_requested_native_artifact_with_checked_boundary_operator_scope,
    terminal_authority_permission_policy_with_rows, terminal_authority_policy_with_rows,
};

#[cfg(test)]
mod tests;
