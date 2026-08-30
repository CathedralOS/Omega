//! Native process enforcement for compiler-owned package-source resolution.
//!
//! The public surface is deliberately narrow: callers select a closed
//! resolution phase, provide already-verified executable and custody paths,
//! and receive opaque policy observations. Implementation lives behind named
//! modules for command construction, confinement, networking, and lifecycle.

#![deny(unsafe_op_in_unsafe_fn)]

mod backend;
mod confinement;
mod model;
mod network;
mod prepared;
mod process;
mod request;

pub use backend::ResolverExecutionBackend;
pub use model::{
    ResolverExecutionBackendIdentity, ResolverExecutionGuarantee,
    ResolverExecutionGuaranteeDisposition, ResolverExecutionGuaranteeRow,
    ResolverExecutionNetworkTransport, ResolverExecutionPhase, ResolverExecutionPolicyObservation,
    ResolverExecutionResourceCeilings,
};
pub use network::{
    RESOLVER_CONNECT_BROKER_ENVIRONMENT, RESOLVER_CONNECT_HELPER_BASENAME,
    RESOLVER_CONNECT_TARGET_ENVIRONMENT, ResolverExecutionEndpointEvent,
    ResolverExecutionEndpointHost, ResolverExecutionEndpointObservation,
    ResolverExecutionEndpointOutcome, ResolverExecutionEndpointRoute,
    ResolverExecutionEndpointRoutePolicy, ResolverExecutionRequestedEndpoint,
    ResolverExecutionTransferBudget, run_resolver_connect_helper,
};
pub use prepared::{ResolverExecutionCommandIdentity, ResolverPreparedExecution};
pub use process::{
    ResolverExecutionChild, ResolverExecutionCompletionObservation, ResolverExecutionExitStatus,
    ResolverExecutionTerminationDisposition,
};
