//! Callable review and checked realization. Build-native supply is reviewed
//! with the exact provider rows, never a synthetic callable.

mod boundary_operators;
mod boundary_requirements;
mod conformance_order;
mod conformances;
mod policy;
mod policy_parameters;
mod reach_dependency;
mod surface;
pub(crate) use policy::project_checked_callable_policy;
