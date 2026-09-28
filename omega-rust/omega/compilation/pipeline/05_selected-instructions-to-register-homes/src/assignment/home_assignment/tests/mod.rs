//! Optimizer module role: stage group. Focused home-assignment behavior taxonomy.

mod determinism;
mod empty_values;
mod fixtures;
mod prepared_conflicts;
mod ties;

pub(super) use super::{compute::compute_function, validate};
