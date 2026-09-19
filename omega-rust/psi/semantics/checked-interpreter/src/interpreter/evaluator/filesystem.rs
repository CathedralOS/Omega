//! Filesystem provider dispatch, rooted output custody, and logical handle tracking.
//! `filesystem_calls.rs` dispatches calls to the selected provider.
//! `observations.rs` and `logical_handles.rs` retain operation custody,
//! and `virtual_filesystem.rs` supports the differential oracle.

mod filesystem_calls;
mod logical_handles;
mod observations;
#[cfg(test)]
mod tests;
mod virtual_filesystem;
