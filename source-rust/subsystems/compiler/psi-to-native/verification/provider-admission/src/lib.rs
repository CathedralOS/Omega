//! Admit the providers reached by the selected entry against the authorized
//! terminal artifact and the receiving authority policy.

#![forbid(unsafe_code)]

mod access_lifecycles;
mod adapters;
mod admission;
mod admission_source;
mod callback_entries;
mod compiler_builtins;
pub mod installed_interrupt_helpers;
mod lowering_facts;
mod model;
mod native_binding_settlements;
pub mod provider_installation;
mod settlements;
mod source_evaluated_bindings;
mod static_interrupts;
mod task_activations;
mod terminal_authority_permission_policy;
mod terminal_authority_policy;
mod terminal_authority_review;

pub(crate) use ::wire::digest_encoder;
pub use adapters::{
    AdmittedProgramProviderContext, retain_selected_provider_adapters,
    selected_synchronous_call_graph,
};
pub(crate) use admission::AdmittedTerminalMechanism;
pub use admission::{
    AdmittedNativeCallableAuthority, AdmittedNativeProviders, ProviderExecutionEvidence,
    admit_native_providers, realization_error,
};
pub use admission_source::{
    AdmissionSource, AdmittedInstalledEntryExecution, AdmittedProgramEntryCall, ComponentCallFacts,
};
pub use callback_entries::project_native_callback_arguments;
pub use compiler_builtins::{
    AdmittedTerminalIntrinsics, CompilerIntrinsicPlanProvenance, SettledCompilerBuiltin,
    TerminalCompilerBuiltinProposal, TerminalCompilerBuiltinSettlement, admit_terminal_intrinsics,
    derive_selected_intrinsic_settlement_proposals, numeric_boundary_execution,
    prepare_retained_compiler_builtins, validate_external_provider_owners,
};
pub use installation_conditions::{
    BootstrapAssumptionInterval, BootstrapEnvironmentAssumption, InstallationEnvironmentAssumption,
    InterruptDeliveryInterval, X86InterruptDeliveryAssumption, X86InterruptDeliveryProfile,
};
pub use lowering_facts::{AdmittedInstalledInterruptCohort, AdmittedNativeCallbackCohort};
pub use model::{
    AdmittedBoundaryExecution, AdmittedBoundarySettlement, AdmittedCallbackEntry,
    AdmittedFatalInterruptEntry, AdmittedInstalledInterruptEntry, AdmittedInstalledInterruptHelper,
    AdmittedInstalledInterruptScope, AdmittedInterruptContextAnchor,
    AdmittedNativeCallbackArgument, InstalledInterruptNativeEntryProposal,
    NativeBoundaryRealization, NativeCallbackArgumentProposal, NativeCompilerBuiltinSettlement,
    NativeProviderSettlement, ProviderAdmissionRequest,
};
pub use provider_installation::{
    AdmittedProviderInstallation, ProviderInstallationError, SelectedProviderAdapter,
};
pub use source_evaluated_bindings::{
    PreparedSourceEvaluatedBindings, SourceEvaluatedImportSettlement,
    prepare_source_evaluated_bindings,
};
pub use task_activations::validate_task_activations;
pub use terminal_authority_permission_policy::{
    TerminalAuthorityPermissionPolicy, TerminalAuthorityPermissionPolicyRow,
    terminal_authority_permission_policy_with_rows,
};
pub use terminal_authority_policy::{
    TerminalAuthorityPolicy, current_terminal_authority_policy, filesystem_host_permission_rows,
    terminal_authority_policy_with_rows, time_host_permission_rows,
};
pub(crate) use terminal_authority_policy::{
    conservative_syscall_terminal_mechanism, normalized_foreign_terminal_mechanism,
    normalized_foreign_terminal_mechanism_with_callback_materializations,
};

pub mod terminal_authority_permissions {
    pub use super::terminal_authority_permission_policy::{
        admit_package_terminal_authority_permissions,
        validate_package_terminal_authority_permission_custody,
        validate_retained_package_terminal_authority_permissions,
    };
}
