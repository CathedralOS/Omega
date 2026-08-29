use std::io;
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use super::budget::ResolverExecutionTransferBudget;
use super::connect_protocol::{ConnectRequestError, read_connect_request, write_response};
use super::model::{
    ResolverExecutionEndpointEvent, ResolverExecutionEndpointObservation,
    ResolverExecutionEndpointOutcome, ResolverExecutionEndpointRoutePolicy,
    ResolverExecutionRequestedEndpoint,
};
use super::relay::relay;

pub(super) const ENDPOINT_CONNECTION_LIMIT: u16 = 16;
pub(super) const DNS_RESULT_LIMIT: usize = 32;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const CONNECT_ATTEMPT_SLICE: Duration = Duration::from_millis(100);
pub(super) const IO_POLL_INTERVAL: Duration = Duration::from_millis(2);

#[derive(Debug)]
struct BrokerState {
    stop: AtomicBool,
    events: Mutex<Vec<ResolverExecutionEndpointEvent>>,
    transfer_budget: ResolverExecutionTransferBudget,
}

/// A compiler-owned, fixed-bound HTTP CONNECT route.
///
/// The route accepts only its typed requested endpoint. Callers receive the
/// loopback proxy address but cannot broaden the broker's destination policy.
#[derive(Debug)]
pub struct ResolverExecutionEndpointRoute {
    policy: ResolverExecutionEndpointRoutePolicy,
    state: Arc<BrokerState>,
    worker: Option<JoinHandle<io::Result<()>>>,
}

impl ResolverExecutionEndpointRoute {
    pub(crate) fn open(
        requested_endpoint: ResolverExecutionRequestedEndpoint,
        transfer_budget: ResolverExecutionTransferBudget,
    ) -> io::Result<Self> {
        // Resolve and bound the complete answer before the broker can open an
        // upstream socket. A successful early address cannot hide an oversized
        // DNS answer suffix.
        let upstream_addresses = Arc::<[SocketAddr]>::from(resolve_upstream(&requested_endpoint)?);
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        listener.set_nonblocking(true)?;
        let broker_endpoint = listener.local_addr()?;
        if !broker_endpoint.ip().is_loopback() {
            return Err(io::Error::other(
                "resolver endpoint broker did not bind to loopback",
            ));
        }
        let policy = ResolverExecutionEndpointRoutePolicy {
            requested_endpoint,
            broker_endpoint,
            connection_limit: ENDPOINT_CONNECTION_LIMIT,
            transfer_byte_ceiling: transfer_budget.ceiling(),
        };
        let state = Arc::new(BrokerState {
            stop: AtomicBool::new(false),
            events: Mutex::new(Vec::with_capacity(usize::from(ENDPOINT_CONNECTION_LIMIT))),
            transfer_budget,
        });
        let thread_policy = policy.clone();
        let thread_state = Arc::clone(&state);
        let worker = thread::Builder::new()
            .name("omega-resolver-connect-broker".to_owned())
            .spawn(move || {
                broker_loop(listener, thread_policy, upstream_addresses, thread_state)
            })?;
        Ok(Self {
            policy,
            state,
            worker: Some(worker),
        })
    }

    pub const fn policy(&self) -> &ResolverExecutionEndpointRoutePolicy {
        &self.policy
    }

    pub fn finish(mut self) -> io::Result<ResolverExecutionEndpointObservation> {
        self.stop_and_join()?;
        let mut events = self
            .state
            .events
            .lock()
            .map_err(|_| io::Error::other("resolver endpoint event lock was poisoned"))?
            .clone();
        events.sort_by_key(ResolverExecutionEndpointEvent::ordinal);
        if events.len() > usize::from(self.policy.connection_limit) {
            return Err(io::Error::other(
                "resolver endpoint broker exceeded its event limit",
            ));
        }
        Ok(ResolverExecutionEndpointObservation {
            route: self.policy.clone(),
            events,
        })
    }

    fn stop_and_join(&mut self) -> io::Result<()> {
        self.state.stop.store(true, Ordering::Release);
        let Some(worker) = self.worker.take() else {
            return Ok(());
        };
        worker
            .join()
            .map_err(|_| io::Error::other("resolver endpoint broker panicked"))?
    }
}

impl Drop for ResolverExecutionEndpointRoute {
    fn drop(&mut self) {
        let _ = self.stop_and_join();
    }
}

fn broker_loop(
    listener: TcpListener,
    policy: ResolverExecutionEndpointRoutePolicy,
    upstream_addresses: Arc<[SocketAddr]>,
    state: Arc<BrokerState>,
) -> io::Result<()> {
    let mut workers = Vec::with_capacity(usize::from(policy.connection_limit));
    let mut accepted = 0_u16;
    while !state.stop.load(Ordering::Acquire) && accepted < policy.connection_limit {
        match listener.accept() {
            Ok((client, _address)) => {
                accepted += 1;
                let ordinal = accepted;
                let worker_policy = policy.clone();
                let worker_addresses = Arc::clone(&upstream_addresses);
                let worker_state = Arc::clone(&state);
                workers.push(thread::spawn(move || {
                    let event = handle_client(
                        client,
                        ordinal,
                        &worker_policy,
                        &worker_addresses,
                        &worker_state.stop,
                        &worker_state.transfer_budget,
                    );
                    if let Ok(mut events) = worker_state.events.lock() {
                        events.push(event);
                    }
                }));
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(IO_POLL_INTERVAL);
            }
            Err(error) => return Err(error),
        }
    }
    drop(listener);
    for worker in workers {
        worker
            .join()
            .map_err(|_| io::Error::other("resolver endpoint connection worker panicked"))?;
    }
    Ok(())
}

fn handle_client(
    mut client: TcpStream,
    ordinal: u16,
    policy: &ResolverExecutionEndpointRoutePolicy,
    upstream_addresses: &[SocketAddr],
    stop: &AtomicBool,
    transfer_budget: &ResolverExecutionTransferBudget,
) -> ResolverExecutionEndpointEvent {
    let request = match read_connect_request(&mut client, stop) {
        Ok(request) => request,
        Err(ConnectRequestError::Malformed) => {
            let _ = write_response(&mut client, b"HTTP/1.1 400 Bad Request\r\n\r\n");
            return endpoint_event(
                ordinal,
                ResolverExecutionEndpointOutcome::MalformedConnect,
                None,
            );
        }
        Err(ConnectRequestError::Oversized) => {
            let _ = write_response(
                &mut client,
                b"HTTP/1.1 431 Request Header Fields Too Large\r\n\r\n",
            );
            return endpoint_event(
                ordinal,
                ResolverExecutionEndpointOutcome::OversizedConnect,
                None,
            );
        }
    };
    if request != policy.requested_endpoint {
        let _ = write_response(&mut client, b"HTTP/1.1 403 Forbidden\r\n\r\n");
        return endpoint_event(
            ordinal,
            ResolverExecutionEndpointOutcome::DestinationRejected,
            None,
        );
    }

    let mut upstream = match connect_upstream(upstream_addresses, stop) {
        Ok(upstream) => upstream,
        Err(_) => {
            let _ = write_response(&mut client, b"HTTP/1.1 502 Bad Gateway\r\n\r\n");
            return endpoint_event(
                ordinal,
                ResolverExecutionEndpointOutcome::UpstreamUnavailable,
                None,
            );
        }
    };
    let effective_peer = upstream.peer_addr().ok();
    if write_response(&mut client, b"HTTP/1.1 200 Connection Established\r\n\r\n").is_ok() {
        let relay = relay(&mut client, &mut upstream, stop, transfer_budget);
        return ResolverExecutionEndpointEvent {
            ordinal,
            outcome: if relay.transfer_ceiling_reached {
                ResolverExecutionEndpointOutcome::TransferCeilingReached
            } else {
                ResolverExecutionEndpointOutcome::Connected
            },
            effective_peer,
            uploaded_bytes: relay.uploaded_bytes,
            downloaded_bytes: relay.downloaded_bytes,
        };
    }
    endpoint_event(
        ordinal,
        ResolverExecutionEndpointOutcome::Connected,
        effective_peer,
    )
}

fn endpoint_event(
    ordinal: u16,
    outcome: ResolverExecutionEndpointOutcome,
    effective_peer: Option<SocketAddr>,
) -> ResolverExecutionEndpointEvent {
    ResolverExecutionEndpointEvent {
        ordinal,
        outcome,
        effective_peer,
        uploaded_bytes: 0,
        downloaded_bytes: 0,
    }
}

fn resolve_upstream(endpoint: &ResolverExecutionRequestedEndpoint) -> io::Result<Vec<SocketAddr>> {
    collect_bounded_addresses(endpoint.authority().to_socket_addrs()?)
}

pub(super) fn collect_bounded_addresses(
    addresses: impl IntoIterator<Item = SocketAddr>,
) -> io::Result<Vec<SocketAddr>> {
    let mut bounded = Vec::with_capacity(DNS_RESULT_LIMIT);
    for address in addresses {
        if bounded.len() == DNS_RESULT_LIMIT {
            return Err(io::Error::other(
                "resolver endpoint DNS result set exceeds its fixed limit",
            ));
        }
        bounded.push(address);
    }
    if bounded.is_empty() {
        return Err(io::Error::other("resolver endpoint resolved no addresses"));
    }
    Ok(bounded)
}

fn connect_upstream(addresses: &[SocketAddr], stop: &AtomicBool) -> io::Result<TcpStream> {
    let deadline = Instant::now() + CONNECT_TIMEOUT;
    let mut last_error = None;
    while Instant::now() < deadline {
        for address in addresses {
            if stop.load(Ordering::Acquire) {
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "resolver endpoint route was stopped",
                ));
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }
            match TcpStream::connect_timeout(address, remaining.min(CONNECT_ATTEMPT_SLICE)) {
                Ok(stream) => return Ok(stream),
                Err(error) => last_error = Some(error),
            }
        }
        thread::sleep(IO_POLL_INTERVAL);
    }
    Err(last_error.unwrap_or_else(|| io::Error::other("resolver endpoint resolved no addresses")))
}
