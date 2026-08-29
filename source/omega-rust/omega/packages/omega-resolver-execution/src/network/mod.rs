mod broker;
mod budget;
mod connect_protocol;
mod helper;
mod model;
mod relay;

pub use broker::ResolverExecutionEndpointRoute;
pub use budget::ResolverExecutionTransferBudget;
pub use helper::{
    RESOLVER_CONNECT_BROKER_ENVIRONMENT, RESOLVER_CONNECT_HELPER_BASENAME,
    RESOLVER_CONNECT_TARGET_ENVIRONMENT, run_resolver_connect_helper,
};
pub use model::{
    ResolverExecutionEndpointEvent, ResolverExecutionEndpointHost,
    ResolverExecutionEndpointObservation, ResolverExecutionEndpointOutcome,
    ResolverExecutionEndpointRoutePolicy, ResolverExecutionRequestedEndpoint,
};

#[cfg(test)]
use {
    broker::{
        DNS_RESULT_LIMIT, ENDPOINT_CONNECTION_LIMIT, IO_POLL_INTERVAL, collect_bounded_addresses,
    },
    connect_protocol::{CONNECT_REQUEST_BYTE_LIMIT, open_connect_tunnel},
    std::{
        io::{Read, Write},
        net::{IpAddr, Ipv4Addr, Shutdown, SocketAddr, TcpListener, TcpStream},
        thread,
        time::{Duration, Instant},
    },
};

#[cfg(test)]
mod tests;
