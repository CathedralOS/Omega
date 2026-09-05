//! Optimizer module role: stage group. Pressure-rematerialization test map.

mod fixtures;
pub(crate) mod multiple_use;
#[cfg(test)]
mod policy_rejection;
pub(crate) mod sole_use;

#[cfg(test)]
pub(crate) use fixtures::{fixture, multiple_future_fixture};
