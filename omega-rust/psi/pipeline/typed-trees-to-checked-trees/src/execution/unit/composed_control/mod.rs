//! Atomic multi-state Unit control plans.
mod assembly;
mod closed_sum;
mod custody;
mod dynamic_join;
mod dynamic_result;
mod guards;
mod leaves;
pub(super) mod topology;

#[cfg(test)]
pub(super) use assembly::build_all as build_checked_composed_unit_control_machines;
pub(super) use assembly::build_all_traced as build_checked_composed_unit_control_machines_traced;
pub(super) use assembly::finish as finish_state_graph;
pub(super) use dynamic_join::{DynamicJoinControlTopology, admit_dynamic_join_control_topology};
pub(crate) use dynamic_result::build as build_direct_dynamic_unit_continuation;
