//! Optimizer module role: stage group. Layout-independent selected-form encoding custody stages.

pub(crate) mod active_resident_selected_form_encoding;
pub(crate) mod post_allocation_selected_form_encoding;

pub use active_resident_selected_form_encoding::*;
pub use post_allocation_selected_form_encoding::*;
