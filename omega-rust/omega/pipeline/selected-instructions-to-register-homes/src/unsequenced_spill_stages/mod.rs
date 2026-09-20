//! Optimizer module role: stage group. Spill boundaries validated but not yet sequenced by register allocation.
//!
//! `crate::register_allocation::stage_register_allocation` calls none of the
//! families below. Executable pressure recovery on the sequenced route is
//! `crate::assignment::runtime_spill`; these families are the bounded,
//! independently replayed spill boundaries that would extend it: abstract
//! spill insertion, reload-value and synthetic reload-value homes, recursive
//! and generalized recovery worklists, victim choices and logical actions,
//! spill-pseudo lowering, and abstract spill memory effects and access
//! constraints. Logical spill planning is sequenced under
//! `crate::assignment::logical_spill_operations` and stack-slot coloring over
//! that plan under `crate::assignment::stack_slot_coloring`.
//!
//! They are exercised by the native-differential `register_allocation` tests
//! and the architecture ladders, and machine emission's non-authoritative
//! spill-frame requirements accept the access-constraint output, but no
//! executable route produces these facts. Sequencing one means calling it from
//! `stage_register_allocation`, not re-declaring it beside the live stages.

pub(crate) mod abstract_spill_access_constraints;
pub(crate) mod abstract_spill_insertion;
pub(crate) mod abstract_spill_memory_effects;
pub(crate) mod generalized_reload_value_homes;
pub(crate) mod generalized_spill_insertion;
pub(crate) mod generalized_spill_recovery_actions;
pub(crate) mod generalized_spill_recovery_choice;
pub(crate) mod generalized_spill_recovery_worklist;
pub(crate) mod recursive_reload_value_homes;
pub(crate) mod recursive_spill_insertion;
pub(crate) mod reload_value_homes;
pub(crate) mod spill_pseudo_instructions;
pub(crate) mod spill_recovery_actions;
pub(crate) mod spill_recovery_choice;
pub(crate) mod spill_recovery_worklist;
pub(crate) mod synthetic_reload_values;
