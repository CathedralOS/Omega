//! Callable review, checked realization, and external supply.

mod boundary_operators;
mod boundary_requirements;
mod conformances;
mod external_supply;
mod review;
mod signatures;

pub(super) use review::{project_callable, project_private_external_executable_supply};
