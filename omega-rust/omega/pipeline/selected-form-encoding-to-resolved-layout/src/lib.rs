#![forbid(unsafe_code)]

//! Optimizer module role: crate map. Selected-form encoding to resolved layout.
//!
//! Required function-relative layout independently checks its result before
//! publication. Optional layout rewrites belong to the following X-to-X phase.
//! Current layout data and content identity belong to machine-code.

mod resolved_selected_form_layout;

pub use resolved_selected_form_layout::*;
