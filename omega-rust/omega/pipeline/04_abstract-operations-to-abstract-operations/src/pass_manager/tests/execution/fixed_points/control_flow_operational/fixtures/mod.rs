//! Optimizer module role: stage group. Isolated control-flow graph fixtures by transformation.

mod branches;
mod machines;
mod merges;
mod threading;

pub(super) use branches::constant_merge_barrier_unit;
pub(super) use machines::unreachable_private_machine_unit;
pub(super) use merges::{isolated_shared_terminal_unit, terminal_non_adjacent_merge_unit};
pub(super) use threading::{linear_shared_target_unit, path_qualified_direct_edges_unit};
