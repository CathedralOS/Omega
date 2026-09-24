//! Filesystem provider dispatch, rooted output custody, and logical handle tracking.
//! `filesystem_calls.rs` dispatches calls to the selected provider.
//! `observations.rs` and `logical_handles.rs` retain operation custody,
//! and `virtual_filesystem.rs` supports the differential oracle.

mod filesystem_calls;
pub(super) mod host_open_flags;
pub(super) mod host_operation;
pub(super) mod logical_handle_store;
mod logical_handles;
mod observations;
pub(super) mod preparation;
/// The REAL-filesystem provider (opt-in `FilesystemAccess::RealUnscoped`; the
/// build.omg rung). A CHILD module so it can serve ops against the private
/// `Evaluator` internals (the fs argument/buffer helpers) without widening
/// their visibility outside the interpreter owner.
pub(in crate::interpreter) mod real;
#[cfg(test)]
mod tests;
mod virtual_filesystem;
