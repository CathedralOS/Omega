//! Callable review, checked realization, and external supply.

mod boundary_operators;
mod boundary_requirements;
mod conformance_order;
mod conformances;
mod external_supply;
mod policy;
mod policy_parameters;
mod review;
mod surface;
pub use policy::project_checked_callable_policy;
mod signatures;

pub(super) use review::{
    project_callable, project_contract_entailment_open_contract,
    project_private_external_executable_supply,
};
