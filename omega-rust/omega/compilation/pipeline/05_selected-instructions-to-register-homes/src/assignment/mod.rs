//! Optimizer module role: stage group. Register homes for admitted selected-program facts.
//!
//! `crate::register_allocation::stage_register_allocation` sequences
//! `transformed` homes after a completed selected-lowering run, the optional
//! `recovery` rules, `runtime_spill` pressure recovery, `baseline` homes over
//! the direct `home_assignment`, and the `post_allocation_manifest` every
//! route publishes. Runtime-spill recovery selects and rewrites physical
//! victims directly. The crate root re-exports each owner's public names by
//! module.

pub(crate) mod baseline;
pub(crate) mod home_assignment;
pub(crate) mod post_allocation_manifest;
pub(crate) mod recovery;
pub(crate) mod runtime_spill;
pub(crate) mod transformed;
