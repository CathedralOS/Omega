//! Optimizer module role: stage group. Resolved selected-form layout, relaxation, and exit-contract stages.

pub(crate) mod whole_function_exit_contract;

pub use omega_selected_form_encoding_to_resolved_layout::*;
pub use whole_function_exit_contract::*;
