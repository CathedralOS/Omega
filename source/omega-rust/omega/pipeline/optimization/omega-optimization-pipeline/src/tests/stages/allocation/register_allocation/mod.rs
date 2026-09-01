//! Optimizer module role: stage group. Register-allocation test group by exact retained artifact family.

mod abstract_spill_insertion;
mod allocation_legality;
mod fixed_view_copies;
mod live_ranges;
mod liveness;
mod logical_spill_operations;
mod register_homes;
mod selected_input;
mod stack_slot_coloring;
