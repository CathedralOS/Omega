//! Optimizer module role: stage group. Register homes for admitted selected-program facts.
//!
//! `crate::register_allocation::stage_register_allocation` sequences
//! `transformed` homes after a completed selected-lowering run, the optional
//! `recovery` rules, `runtime_spill` pressure recovery, `baseline` homes over
//! the direct `home_assignment`, and the `post_allocation_manifest` every
//! route publishes. `logical_spill_operations` and `stack_slot_coloring`
//! remain validated boundaries with no sequenced caller: runtime-spill
//! recovery selects and rewrites physical victims directly rather than
//! planning over them, so only the native-differential boundary tests and
//! the `crate::unsequenced_spill_stages` compositions consuming their
//! receipts exercise them. The crate root re-exports each owner's public
//! names by module.

pub(crate) mod baseline;
pub(crate) mod home_assignment;
pub(crate) mod logical_spill_operations;
pub(crate) mod post_allocation_manifest;
pub(crate) mod recovery;
pub(crate) mod runtime_spill;
pub(crate) mod stack_slot_coloring;
pub(crate) mod transformed;
