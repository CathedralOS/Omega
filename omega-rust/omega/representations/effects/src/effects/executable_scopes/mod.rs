//! Executable scopes: the eras that share one process, the scopes admitted
//! as isolated, each scope's trusted computing base and the allowance it is
//! judged against, and process-wide service registrations.

pub(crate) mod coexisting_executable_eras;
pub(crate) mod executable_tcb_manifest;
pub(crate) mod executable_tcb_profile;
pub(crate) mod isolated_executable_scopes;
pub(crate) mod process_static_services;
