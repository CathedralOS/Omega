//! Optimizer module role: stage group. Focused home-assignment behavior taxonomy.

mod constrained_domains;
mod determinism;
mod early_clobber;
mod empty_values;
mod fixtures;
mod placement;
mod prepared_conflicts;
mod ties;

pub(super) use super::{compute::compute_function, validate};
