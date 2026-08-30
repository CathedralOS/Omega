use crate::{
    ResolverExecutionBackend, ResolverExecutionEndpointOutcome, ResolverExecutionEndpointRoute,
    ResolverExecutionGuarantee, ResolverExecutionGuaranteeDisposition,
    ResolverExecutionNetworkTransport, ResolverExecutionPhase, ResolverExecutionPolicyObservation,
    ResolverExecutionRequestedEndpoint, ResolverExecutionTransferBudget,
};
use std::path::{Path, PathBuf};
use std::process::Stdio;

fn loopback_route(backend: &ResolverExecutionBackend) -> ResolverExecutionEndpointRoute {
    backend
        .open_endpoint_route(
            ResolverExecutionRequestedEndpoint::new("127.0.0.1", 9)
                .expect("construct loopback endpoint"),
            ResolverExecutionTransferBudget::new(1024 * 1024).expect("construct transfer budget"),
        )
        .expect("open loopback endpoint route")
}

fn inspection_root() -> PathBuf {
    std::env::temp_dir()
        .canonicalize()
        .expect("canonical temporary inspection root")
}

mod https_confinement;
mod initialization;
mod inspection;
mod network;
mod policy_observation;
