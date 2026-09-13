//! Windows Job Object resource limits and process-tree lifecycle.
//!
//! Launch setup creates and configures a compiler-owned Job Object before the
//! child is released. Lifecycle tracking does not report completion until the
//! complete descendant process tree has exited.

mod job;
mod launch;
mod lifecycle;
mod limits;
mod native;
mod termination;

pub(crate) use lifecycle::WindowsJobChild;

#[cfg(test)]
mod tests;
