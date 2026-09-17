//! Atomic multi-state Unit control plans.
#[path = "assembly.rs"]
mod assembly;
#[path = "closed_sum.rs"]
mod closed_sum;
#[path = "custody.rs"]
mod custody;
#[path = "dynamic_join.rs"]
mod dynamic_join;
#[path = "dynamic_result.rs"]
mod dynamic_result;
#[path = "guards.rs"]
mod guards;
#[path = "leaves.rs"]
mod leaves;
#[path = "nested_control/build_control.rs"]
mod nested_control;
#[path = "prefixed_control.rs"]
mod prefixed_control;
#[path = "topology.rs"]
pub(super) mod topology;

#[cfg(test)]
pub(super) use assembly::build_all as build_checked_composed_unit_control_machines;
pub(super) use assembly::build_all_traced as build_checked_composed_unit_control_machines_traced;
pub(super) use assembly::finish as finish_state_graph;
pub(super) use dynamic_join::{DynamicJoinControlTopology, admit_dynamic_join_control_topology};
pub(crate) use dynamic_result::build as build_direct_dynamic_unit_continuation;
