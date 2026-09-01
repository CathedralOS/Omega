#![forbid(unsafe_code)]

//! Optimizer module role: crate map. Shared composition from canonical Terminal Psi to a replayed native artifact.
//!
//! This crate is named for its exact input and output. Its entrances settle
//! source-entry custody, coordinate ordinary or explicitly selected native
//! realization, and own the ProgramStorage semantic-wrapper join. It does not
//! own source compilation, component policy, installation, or publication.

mod entry_settlement;
mod optimized_semantic_wrapper_encoding;
mod optimized_semantic_wrapper_object;
mod realization;

pub use entry_settlement::{
    NativeProgramEntrySettlement, NativeProgramEntrySettlementError,
    ValidatedNativeProgramEntrySettlement, validate_native_program_entry_settlement,
};
pub use omega_abstract_operations_to_target_operations::AdmittedIeeeFloatFmaSettlement;
pub use omega_native_artifact::{
    BoundaryExecutionRecord, BoundaryTraitSettlement, BoundaryTraitSettlementParts, NativeArtifact,
    NativeArtifactParts, NativeByteSpan, NativePhysicalChild, NativePhysicalChildParts,
    NativePhysicalEvidence, NativePhysicalEvidenceParts, NativePhysicalEvidenceScope,
    NativePhysicalOccurrence, NativeProviderExecution, NativeSelectedProviderPlan,
    NativeSelectedProviderPlanDigest, PhysicalChildParent, PhysicalRelocationDisposition,
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
    NativeProviderSettlement, NativeRealizationRequest, RealizedNativeArtifactWithCallbackCustody,
    SettledNativeArtifact, TERMINAL_AUTHORITY_PERMISSION_POLICY_VERSION,
    TERMINAL_AUTHORITY_POLICY_VERSION, TerminalAuthorityPermissionPolicy,
    TerminalAuthorityPermissionPolicyBuildError, TerminalAuthorityPermissionPolicyRow,
    TerminalAuthorityPolicy, TerminalAuthorityPolicyBuildError, TerminalAuthorityPolicyRow,
    UnclassifiedCompilerIntrinsicTerminalMechanism, UnclassifiedTerminalMechanism,
    current_compiler_intrinsic_terminal_authority_policy,
    current_terminal_authority_permission_policy, current_terminal_authority_policy,
    normalized_foreign_terminal_mechanism,
    normalized_foreign_terminal_mechanism_with_callback_materializations, realize_native_artifact,
    realize_native_artifact_with_callback_custody,
    realize_native_artifact_with_checked_boundary_operator_scope,
    realize_program_entry_native_artifact, terminal_authority_permission_policy_with_rows,
    terminal_authority_policy_with_rows,
};

#[cfg(test)]
mod tests;
