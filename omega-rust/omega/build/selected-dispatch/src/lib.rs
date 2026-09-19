#![forbid(unsafe_code)]

//! Checked-Psi dispatch settlement for exact build-selected realizations.
//!
//! The compiler coordinates these rewrites after checking. This crate owns
//! their semantics and atomic plan/apply behavior. Start at `selected_dispatch.rs`
//! for execution settlement or `boundary_dispatch.rs` for call associations.

mod boundary_dispatch;
mod compiler_intrinsic;
mod intrinsic_review;
mod selected_dispatch;
mod service_custody;
mod source_edits;

pub use boundary_dispatch::{
    selected_boundary_family_specializations, settle_selected_boundary_adapter_dispatch,
};
pub use compiler_intrinsic::{
    derive_selected_compiler_intrinsic_execution_identity_for_row,
    derive_selected_compiler_intrinsic_execution_identity_for_row_with_binding,
    derive_selected_compiler_intrinsic_execution_identity_for_row_with_resolved_binding,
};

pub use intrinsic_review::{
    ResolvedAcceptedSemanticBinding, resolve_accepted_service_binding,
    retain_selected_compiler_intrinsic_review_identities,
};

pub use service_custody::{
    derive_fused_program_entry_establishments, validate_fused_service_terminal_custody,
};
pub use source_edits::SelectedDispatchSourceEdits;

pub use selected_dispatch::{
    CheckedNongenericOperatorApplicationRealization, CheckedOperatorAuthoredUseKind,
    CheckedSpecializedOperatorApplicationRealization, SelectedCompilerIntrinsicExecutionIdentity,
    derive_checked_nongeneric_operator_application_realizations,
    derive_checked_specialized_operator_application_realizations,
    derive_selected_compiler_intrinsic_execution_identity,
    derive_selected_primitive_float_binary_execution, settle_selected_execution_dispatch,
    settle_selected_execution_dispatch_with_source_edits, settle_selected_float_intrinsic_dispatch,
    validate_selected_operator_terminal_custody,
};
