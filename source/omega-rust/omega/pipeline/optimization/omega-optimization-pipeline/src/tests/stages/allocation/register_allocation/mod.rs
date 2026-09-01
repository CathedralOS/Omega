//! Optimizer module role: stage group. Register-allocation test group by exact retained artifact family.

mod abstract_spill_insertion;
mod allocation_legality;
mod fixed_view_copies;
mod fixed_view_copy_operational;
mod generalized_reload_value_homes;
mod generalized_spill_insertion;
mod generalized_spill_recovery_actions;
mod generalized_spill_recovery_choice;
mod generalized_spill_recovery_worklist;
mod live_ranges;
mod liveness;
mod logical_spill_operations;
mod recursive_spill_insertion;
mod register_homes;
mod reload_value_homes;
mod selected_input;
mod spill_pseudo_instructions;
mod spill_recovery_actions;
mod spill_recovery_choice;
mod spill_recovery_worklist;
mod stack_slot_coloring;
mod synthetic_reload_values;
