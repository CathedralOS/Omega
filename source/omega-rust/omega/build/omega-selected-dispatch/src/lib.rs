#![forbid(unsafe_code)]

//! Checked-Psi dispatch settlement for exact build-selected realizations.
//!
//! The compiler coordinates these rewrites after checking. This crate owns
//! their semantics and atomic plan/apply behavior.

mod adapter;
mod float_intrinsic;
mod operator_adapter;

pub use adapter::settle_selected_boundary_adapter_dispatch;
pub use float_intrinsic::settle_selected_float_intrinsic_dispatch;
pub use operator_adapter::settle_selected_operator_adapter_dispatch;
