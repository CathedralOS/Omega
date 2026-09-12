//! Optimizer module role: executable entrance. Validate custody, admit providers, emit, and replay.

mod boundary_applications;
mod callback_custody;
mod diagnostics;
mod input;
mod model;
mod native_artifact;
mod object;
mod optimization_stage;
mod optimized_fragment_projection;
mod output;
mod physical_stage;
mod program_entry;
pub(crate) mod providers;
mod target_stage;
mod terminal_authority_permission_policy;
mod terminal_authority_policy;
mod terminal_authority_review;

pub use callback_custody::{
    CallbackCustodyNativeRealizationError, RealizedNativeArtifactWithCallbackCustody,
    realize_native_artifact_with_callback_custody,
};
pub use input::{PreparedNativeRealizationInput, prepare_native_realization_input};
pub use model::{
    NativeBoundaryRealization, NativeCallbackThunkSettlement, NativeCompilerBuiltinSettlement,
    NativeProviderSettlement, NativeRealizationRequest, RequestedNativeArtifact,
    RequestedNativeArtifactError, SettledNativeArtifact,
};
pub use native_artifact::realize_native_artifact;
pub use program_entry::realize_program_entry_native_artifact;
pub use terminal_authority_permission_policy::{
    MissingTerminalAuthorityPermission, TERMINAL_AUTHORITY_PERMISSION_POLICY_VERSION,
    TerminalAuthorityPermissionPolicy, TerminalAuthorityPermissionPolicyBuildError,
    TerminalAuthorityPermissionPolicyRow, current_terminal_authority_permission_policy,
    terminal_authority_permission_policy_with_rows,
};
pub use terminal_authority_policy::{
    COMPILER_INTRINSIC_TERMINAL_AUTHORITY_POLICY_VERSION, CompilerIntrinsicTerminalAuthorityPolicy,
    TERMINAL_AUTHORITY_POLICY_VERSION, TerminalAuthorityPolicy, TerminalAuthorityPolicyBuildError,
    TerminalAuthorityPolicyRow, UnclassifiedCompilerIntrinsicTerminalMechanism,
    UnclassifiedTerminalMechanism, conservative_syscall_terminal_mechanism,
    current_compiler_intrinsic_terminal_authority_policy, current_terminal_authority_policy,
    normalized_foreign_terminal_mechanism,
    normalized_foreign_terminal_mechanism_with_callback_materializations,
    terminal_authority_policy_with_rows,
};
