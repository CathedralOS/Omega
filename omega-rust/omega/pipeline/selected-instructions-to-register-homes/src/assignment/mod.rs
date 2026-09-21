//! Optimizer module role: stage group. Register homes for admitted selected-program facts.
//!
//! Every module here is sequenced by
//! `crate::register_allocation::stage_register_allocation`: `transformed`
//! homes after a completed selected-lowering run, the optional `recovery`
//! rules, `runtime_spill` pressure recovery, `baseline` homes over the direct
//! `home_assignment`, and the `post_allocation_manifest` every route
//! publishes. `logical_spill_operations` is the sequenced spill boundary the
//! runtime-spill recovery plans over its input facts, and
//! `stack_slot_coloring` is the sequenced boundary assigning spill-area-relative
//! storage to that plan. The crate root re-exports each owner's public names by module.
//! Spill boundaries that route does not yet call live in
//! `crate::unsequenced_spill_stages`.

pub(crate) mod baseline;
pub(crate) mod home_assignment;
pub(crate) mod logical_spill_operations;
pub(crate) mod post_allocation_manifest;
pub(crate) mod recovery;
pub(crate) mod runtime_spill;
pub(crate) mod stack_slot_coloring;
pub(crate) mod transformed;
