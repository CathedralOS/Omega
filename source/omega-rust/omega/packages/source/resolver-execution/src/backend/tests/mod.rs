use super::{ResolverExecutionAuthorityRoots, ResolverExecutionBackend};
use crate::{
    ResolverExecutionEndpointRoute, ResolverExecutionGuaranteeDisposition,
    ResolverExecutionNetworkTransport, ResolverExecutionPhase, ResolverExecutionRequestedEndpoint,
    ResolverExecutionTransferBudget,
};
use std::path::Path;

mod observations;
#[cfg(unix)]
mod unix;
mod validation;

fn loopback_route(backend: &ResolverExecutionBackend) -> ResolverExecutionEndpointRoute {
    backend
        .open_endpoint_route(
            ResolverExecutionRequestedEndpoint::new("127.0.0.1", 9)
                .expect("construct loopback endpoint"),
            ResolverExecutionTransferBudget::new(1024 * 1024).expect("construct transfer budget"),
        )
        .expect("open loopback endpoint route")
}

fn inspection_root() -> std::path::PathBuf {
    std::env::temp_dir()
        .canonicalize()
        .expect("canonical temporary inspection root")
}
