//! Optimizer module role: stage group. Pressure-rematerialization test map.

mod fixtures;
mod multiple_use;
mod policy_rejection;
mod sole_use;

pub(crate) use fixtures::{fixture, multiple_future_fixture};
