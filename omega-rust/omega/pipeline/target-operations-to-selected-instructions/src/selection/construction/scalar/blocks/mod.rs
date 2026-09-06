//! Optimizer module role: stage group. Shared block constructors; family leaves own their sequencing and IDs.

mod binary;
mod entry;
mod terminal;

pub(super) use self::terminal::{constant_return, parameter_return};
pub(super) use binary::exact_binary_return;
pub(super) use entry::condition;
