#![forbid(unsafe_code)]

//! Checked-Psi dispatch settlement for exact build-selected realizations.
//!
//! Start at `provider_checking.rs` for build-selected checking inputs, then
//! `execution_settlement.rs` for dispatch, accepted services and activation.
//! Psi owns source checking and execution plans. This crate owns realization
//! associations, not another entrance into source checking.

mod boundary_dispatch;
mod compiler_intrinsic;
mod constant_evaluation;
#[cfg(test)]
mod core_service_fixture;
mod execution_settlement;
mod intrinsic_review;
mod provider_checking;
mod selected_dispatch;
mod service_custody;

pub use boundary_dispatch::settle_selected_boundary_adapter_dispatch;
pub use constant_evaluation::SelectedConstantEvaluation;

pub use compiler_intrinsic::{
    derive_selected_compiler_intrinsic_execution_identity_for_row,
    derive_selected_compiler_intrinsic_execution_identity_for_row_with_binding,
    derive_selected_compiler_intrinsic_execution_identity_for_row_with_resolved_binding,
};
pub use execution_settlement::{
    SelectedExecution, SelectedExecutionRequest, settle_selected_execution,
};
pub use provider_checking::{
    CheckedProviderProgram, ProviderCheckingInputs, ProviderCheckingRequest,
};

pub use intrinsic_review::{
    ResolvedAcceptedSemanticBinding, resolve_accepted_service_binding,
    retain_selected_compiler_intrinsic_review_identities,
};

pub use service_custody::{
    derive_fused_program_entry_establishments, program_entry_service_requirements,
    validate_fused_service_terminal_custody,
};

pub use selected_dispatch::{
    CheckedNongenericOperatorApplicationRealization, CheckedOperatorAuthoredUseKind,
    CheckedSpecializedOperatorApplicationRealization, SelectedCompilerIntrinsicExecutionIdentity,
    derive_checked_nongeneric_operator_application_realizations,
    derive_checked_specialized_operator_application_realizations,
    derive_selected_compiler_intrinsic_execution_identity,
    derive_selected_primitive_float_binary_execution, settle_selected_comparison_execution,
    settle_selected_execution_dispatch, validate_selected_operator_terminal_custody,
};
