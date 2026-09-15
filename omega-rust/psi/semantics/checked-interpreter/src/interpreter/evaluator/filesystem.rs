//!
//! `filesystem_calls.rs` takes and dispatches each call, `replay.rs` replays
//! recorded ones, `observations.rs` and `logical_handles.rs` record what a
//! call saw, and `virtual_filesystem.rs` is the store it runs against.

mod filesystem_calls;
mod logical_handles;
mod observations;
mod replay;
#[cfg(test)]
mod tests;
mod virtual_filesystem;
