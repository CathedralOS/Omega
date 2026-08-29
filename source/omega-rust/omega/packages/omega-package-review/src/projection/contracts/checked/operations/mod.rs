mod invocation;
mod suspension;

pub(crate) use invocation::{
    canonical_checked_invocation_targets, project_machine_invocation_source_locations,
    project_signature_invocation_source_locations,
};
pub(crate) use suspension::{
    project_machine_operational_source_locations, project_signature_operational_source_locations,
};
