#![forbid(unsafe_code)]

//! Checked-Psi dispatch settlement for exact build-selected realizations.
//!
//! Start at `provider_binding.rs` for binding immutable checked input, then
//! `execution_settlement.rs` for dispatch, accepted services and activation.
//! Psi owns source checking and execution plans. This crate owns realization
//! associations, not another entrance into source checking.

mod boundary_dispatch;
mod checked_body;

pub(crate) use checked_body::exact_checked_body_entry;
mod compiler_intrinsic;
mod constant_evaluation;
mod execution_settlement;
mod intrinsic_review;
mod provider_binding;
mod selected_dispatch;
mod service_custody;

pub use constant_evaluation::SelectedConstantEvaluation;

pub use compiler_intrinsic::derive_selected_compiler_intrinsic_execution_identity_for_row;
pub use execution_settlement::{
    SelectedExecution, SelectedExecutionRequest, settle_selected_execution,
};
pub(crate) use intrinsic_review::{
    ResolvedAcceptedSemanticBinding, resolve_accepted_service_binding,
    retain_selected_compiler_intrinsic_review_identities,
};
pub use provider_binding::{
    CheckedProviderProgram, ProviderBindingRequest, bind_checked_providers,
};

pub use service_custody::program_entry_service_requirements;

pub use selected_dispatch::{
    CheckedOperatorAuthoredUseKind, SelectedCompilerIntrinsicExecutionIdentity,
    SelectedExecutionDispatch, derive_checked_nongeneric_operator_application_realizations,
    derive_checked_specialized_operator_application_realizations,
    settle_selected_comparison_execution,
};
pub(crate) use selected_dispatch::{
    derive_selected_compiler_intrinsic_execution_identity,
    derive_selected_primitive_float_binary_execution, settle_selected_execution_dispatch,
};
