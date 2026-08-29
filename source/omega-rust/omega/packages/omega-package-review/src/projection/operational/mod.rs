//! Projection of checked operational effects into package-review evidence.

mod crash;
mod flow;
mod invocations;
mod mutation;
mod reach;
mod termination;

pub(crate) use crash::{project_crash, project_crash_routes};
pub(crate) use flow::project_capability_flow;
pub(crate) use invocations::project_synchronous_invocations;
pub(crate) use mutation::project_mutation;
pub(crate) use reach::{project_installation_reaches, project_service_row};
pub(crate) use termination::{
    project_machine_parameter_termination, project_termination,
    project_trait_requirement_termination,
};
