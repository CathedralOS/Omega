//! Optimizer module role: stage group. Resolved selected-form layout, relaxation, and exit-contract stages.

pub(crate) mod resolved_selected_form_layout;
pub(crate) mod whole_function_exit_contract;
pub(crate) mod x86_branch_relaxation;

pub use resolved_selected_form_layout::*;
pub use whole_function_exit_contract::*;
pub use x86_branch_relaxation::*;
