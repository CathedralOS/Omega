//! Derived compiler reports and their validation.

mod checked_observations;
mod production_subject;
pub(super) mod wire;

pub(crate) use checked_observations::{CheckedObservationInput, report_checked_observations};
pub(crate) use production_subject::project_production_subject;
