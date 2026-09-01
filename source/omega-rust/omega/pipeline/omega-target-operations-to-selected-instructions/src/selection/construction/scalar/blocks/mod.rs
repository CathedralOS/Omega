//! Optimizer module role: stage group. Shared block constructors; family leaves own their sequencing and IDs.

mod active_resident;
mod active_resident_bridge;
mod binary;
mod entry;
mod terminal;

pub(super) use active_resident::active_resident_exact_add_chain;
pub(super) use active_resident_bridge::active_resident_exact_add_bridge_chain;
pub(super) use binary::exact_binary_return;
pub(super) use entry::condition;
pub(super) use terminal::{constant_return, parameter_return};
