#![forbid(unsafe_code)]

//! Checked-Psi dispatch settlement for exact build-selected realizations.
//!
//! The compiler coordinates these rewrites after checking. This crate owns
//! their semantics and atomic plan/apply behavior.

mod adapter;
mod compiler_intrinsic;
mod float_intrinsic;
mod intrinsic_review;
mod operator_adapter;

pub use adapter::settle_selected_boundary_adapter_dispatch;
pub use compiler_intrinsic::derive_selected_compiler_intrinsic_execution_identity_for_row;
pub use float_intrinsic::{
    SelectedCompilerIntrinsicExecutionIdentity,
    derive_selected_compiler_intrinsic_execution_identity,
    settle_selected_float_intrinsic_dispatch,
};
pub use intrinsic_review::retain_selected_compiler_intrinsic_review_identities;
pub use operator_adapter::{
    settle_selected_operator_adapter_dispatch, validate_selected_operator_terminal_custody,
};
