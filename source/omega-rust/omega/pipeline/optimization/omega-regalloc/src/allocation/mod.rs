//! Optimizer module role: stage group. Decisions that assign homes or select values requiring recovery.

pub(crate) mod abstract_spill_insertion;
pub(crate) mod generalized_reload_value_homes;
pub(crate) mod generalized_spill_insertion;
pub(crate) mod home_assignment;
pub(crate) mod logical_spill_operations;
mod post_allocation_manifest;
pub(crate) mod reload_value_homes;
pub(crate) mod spill_choice;
pub(crate) mod spill_recovery_actions;
pub(crate) mod spill_recovery_choice;
pub(crate) mod spill_recovery_worklist;
pub(crate) mod stack_slot_coloring;
pub(crate) mod synthetic_reload_values;

pub use abstract_spill_insertion::*;
pub use generalized_reload_value_homes::*;
pub use generalized_spill_insertion::*;
pub use home_assignment::*;
pub use logical_spill_operations::*;
pub use post_allocation_manifest::*;
pub use reload_value_homes::*;
pub use spill_choice::*;
pub use spill_recovery_actions::*;
pub use spill_recovery_choice::*;
pub use spill_recovery_worklist::*;
pub use stack_slot_coloring::*;
pub use synthetic_reload_values::*;
