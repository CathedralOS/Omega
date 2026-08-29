//! Derived compiler reports and their validation.

mod checked_observations;
pub(super) mod wire;

pub(crate) use checked_observations::{CheckedObservationInput, report_checked_observations};
