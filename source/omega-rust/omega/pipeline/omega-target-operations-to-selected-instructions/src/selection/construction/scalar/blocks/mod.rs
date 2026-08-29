//! Shared block constructors; family leaves own their sequencing and IDs.

mod active_resident;
mod binary;
mod entry;
mod terminal;

pub(super) use active_resident::active_resident_exact_add_chain;
pub(super) use binary::exact_binary_return;
pub(super) use entry::condition;
pub(super) use terminal::{constant_return, parameter_return};
