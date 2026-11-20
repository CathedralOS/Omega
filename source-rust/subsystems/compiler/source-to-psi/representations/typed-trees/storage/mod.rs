//! Ordered declaration roots, arena storage and frozen source correspondence.

pub mod declaration_projection;
pub(crate) mod declaration_tables_and_index;
pub(crate) mod source_review;
pub(crate) mod frozen;
pub(crate) mod selection_exclusions;
pub(crate) mod symbol_rebinding;
mod arena_tables;
mod forest;
#[cfg(test)]
mod continuation_storage_tests;
pub use arena_tables::{TypedTreeTables, TypedTreeRoots};
