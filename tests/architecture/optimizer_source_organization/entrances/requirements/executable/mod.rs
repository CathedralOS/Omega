//! Executable coordination seams, partitioned by compiler responsibility.

pub(super) mod compiler;
pub(super) mod machine;
pub(super) mod pipeline_native;
pub(super) mod psi;
pub(super) mod selection_allocation;
pub(super) mod tooling;
pub(super) mod translation;
