use std::io::{self, Read, Write};
use std::net::{IpAddr, Ipv4Addr, Shutdown, SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const CONNECT_REQUEST_BYTE_LIMIT: usize = 4 * 1024;
const CONNECT_HEADER_LIMIT: usize = 32;
const ENDPOINT_CONNECTION_LIMIT: u16 = 16;
const DNS_RESULT_LIMIT: usize = 32;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const CONNECT_ATTEMPT_SLICE: Duration = Duration::from_millis(100);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
const RELAY_TIMEOUT: Duration = Duration::from_secs(120);
const IO_POLL_INTERVAL: Duration = Duration::from_millis(2);

const ENDPOINT_OBSERVATION_SCHEMA_VERSION: u32 = 2;
const ENDPOINT_OBSERVATION_CANONICAL_BYTE_LIMIT: usize = 128 * 1024;
const CONNECT_RESPONSE_BYTE_LIMIT: usize = 4 * 1024;

pub const RESOLVER_CONNECT_HELPER_BASENAME: &str = "omega-resolver-connect";
pub const RESOLVER_CONNECT_BROKER_ENVIRONMENT: &str = "OMEGA_RESOLVER_BROKER";
pub const RESOLVER_CONNECT_TARGET_ENVIRONMENT: &str = "OMEGA_RESOLVER_TARGET";

/// One normalized host named by an already-validated source locator.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ResolverExecutionEndpointHost {
    DnsName(String),
    IpAddress(IpAddr),
}

impl ResolverExecutionEndpointHost {
    pub fn as_str(&self) -> String {
        match self {
            Self::DnsName(name) => name.clone(),
            Self::IpAddress(address) => address.to_string(),
        }
    }

    pub(crate) fn encode(&self, bytes: &mut Vec<u8>) {
        match self {
            Self::DnsName(name) => {
                bytes.push(1);
                encode_bytes(bytes, name.as_bytes());
            }
            Self::IpAddress(IpAddr::V4(address)) => {
                bytes.push(2);
                bytes.extend_from_slice(&address.octets());
            }
            Self::IpAddress(IpAddr::V6(address)) => {
                bytes.push(3);
                bytes.extend_from_slice(&address.octets());
            }
        }
    }
}

/// The sole remote destination authorized for one resolver network command.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ResolverExecutionRequestedEndpoint {
    host: ResolverExecutionEndpointHost,
    port: u16,
}

impl ResolverExecutionRequestedEndpoint {
    pub fn new(host: &str, port: u16) -> io::Result<Self> {
        if port == 0 {
            return Err(invalid_input("resolver endpoint port is zero"));
        }
        let host = normalize_host(host)?;
        Ok(Self { host, port })
    }

    pub const fn host(&self) -> &ResolverExecutionEndpointHost {
        &self.host
    }

    pub const fn port(&self) -> u16 {
        self.port
    }

    pub fn authority(&self) -> String {
        match &self.host {
            ResolverExecutionEndpointHost::DnsName(name) => format!("{name}:{}", self.port),
            ResolverExecutionEndpointHost::IpAddress(IpAddr::V4(address)) => {
                format!("{address}:{}", self.port)
            }
            ResolverExecutionEndpointHost::IpAddress(IpAddr::V6(address)) => {
                format!("[{address}]:{}", self.port)
            }
        }
    }

    fn encode(&self, bytes: &mut Vec<u8>) {
        self.host.encode(bytes);
        bytes.extend_from_slice(&self.port.to_le_bytes());
    }
}

/// Fixed route policy bound into command-construction evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolverExecutionEndpointRoutePolicy {
    requested_endpoint: ResolverExecutionRequestedEndpoint,
    broker_endpoint: SocketAddr,
    connection_limit: u16,
    transfer_byte_ceiling: u64,
}

impl ResolverExecutionEndpointRoutePolicy {
    pub const fn requested_endpoint(&self) -> &ResolverExecutionRequestedEndpoint {
        &self.requested_endpoint
    }

    pub const fn broker_endpoint(&self) -> SocketAddr {
        self.broker_endpoint
    }

    pub const fn connection_limit(&self) -> u16 {
        self.connection_limit
    }

    pub const fn transfer_byte_ceiling(&self) -> u64 {
        self.transfer_byte_ceiling
    }

    pub fn http_proxy_url(&self) -> String {
        format!("http://{}", self.broker_endpoint)
    }

    pub(crate) fn encode(&self, bytes: &mut Vec<u8>) {
        self.requested_endpoint.encode(bytes);
        encode_socket_address(bytes, self.broker_endpoint);
        bytes.extend_from_slice(&self.connection_limit.to_le_bytes());
        bytes.extend_from_slice(&self.transfer_byte_ceiling.to_le_bytes());
    }
}

/// Closed result of one accepted broker connection attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolverExecutionEndpointOutcome {
    Connected,
    MalformedConnect,
    OversizedConnect,
    DestinationRejected,
    UpstreamUnavailable,
    TransferCeilingReached,
}

impl ResolverExecutionEndpointOutcome {
    const fn tag(self) -> u8 {
        match self {
            Self::Connected => 1,
            Self::MalformedConnect => 2,
            Self::OversizedConnect => 3,
            Self::DestinationRejected => 4,
            Self::UpstreamUnavailable => 5,
            Self::TransferCeilingReached => 6,
        }
    }
}

/// One bounded broker event. An effective peer exists only after an actual
/// upstream socket was connected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolverExecutionEndpointEvent {
    ordinal: u16,
    outcome: ResolverExecutionEndpointOutcome,
    effective_peer: Option<SocketAddr>,
    uploaded_bytes: u64,
    downloaded_bytes: u64,
}

impl ResolverExecutionEndpointEvent {
    pub const fn ordinal(&self) -> u16 {
        self.ordinal
    }

    pub const fn outcome(&self) -> ResolverExecutionEndpointOutcome {
        self.outcome
    }

    pub const fn effective_peer(&self) -> Option<SocketAddr> {
        self.effective_peer
    }

    pub const fn uploaded_bytes(&self) -> u64 {
        self.uploaded_bytes
    }

    pub const fn downloaded_bytes(&self) -> u64 {
        self.downloaded_bytes
    }
}

/// Bounded endpoint evidence emitted after a route is closed.
///
/// This records broker activity. It does not establish TLS or SSH trust,
/// credential custody, transferred-byte accounting, or source acceptance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolverExecutionEndpointObservation {
    route: ResolverExecutionEndpointRoutePolicy,
    events: Vec<ResolverExecutionEndpointEvent>,
}

impl ResolverExecutionEndpointObservation {
    pub const fn route(&self) -> &ResolverExecutionEndpointRoutePolicy {
        &self.route
    }

    pub fn events(&self) -> &[ResolverExecutionEndpointEvent] {
        &self.events
    }

    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"omega-resolver-execution-endpoints\0");
        bytes.extend_from_slice(&ENDPOINT_OBSERVATION_SCHEMA_VERSION.to_le_bytes());
        self.route.encode(&mut bytes);
        bytes.extend_from_slice(&(self.events.len() as u16).to_le_bytes());
        for event in &self.events {
            bytes.extend_from_slice(&event.ordinal.to_le_bytes());
            bytes.push(event.outcome.tag());
            match event.effective_peer {
                Some(address) => {
                    bytes.push(1);
                    encode_socket_address(&mut bytes, address);
                }
                None => bytes.push(0),
            }
            bytes.extend_from_slice(&event.uploaded_bytes.to_le_bytes());
            bytes.extend_from_slice(&event.downloaded_bytes.to_le_bytes());
        }
        assert!(bytes.len() <= ENDPOINT_OBSERVATION_CANONICAL_BYTE_LIMIT);
        bytes
    }
}

#[derive(Debug)]
struct BrokerState {
    stop: AtomicBool,
    events: Mutex<Vec<ResolverExecutionEndpointEvent>>,
    transfer_budget: ResolverExecutionTransferBudget,
}

/// One compiler-owned aggregate byte budget shared by every endpoint route in
/// a source resolution. The counter covers tunneled bytes accepted by the
/// broker in both directions; CONNECT framing is excluded.
#[derive(Debug, Clone)]
pub struct ResolverExecutionTransferBudget {
    ceiling: u64,
    observed: Arc<AtomicU64>,
}

impl ResolverExecutionTransferBudget {
    pub fn new(ceiling: u64) -> io::Result<Self> {
        if ceiling == 0 {
            return Err(invalid_input("resolver transfer byte ceiling is zero"));
        }
        Ok(Self {
            ceiling,
            observed: Arc::new(AtomicU64::new(0)),
        })
    }

    pub const fn ceiling(&self) -> u64 {
        self.ceiling
    }

    pub fn observed(&self) -> u64 {
        self.observed.load(Ordering::Acquire)
    }

    fn charge(&self, count: usize) -> Result<(), ()> {
        let count = u64::try_from(count).map_err(|_| ())?;
        self.observed
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                current
                    .checked_add(count)
                    .filter(|attempted| *attempted <= self.ceiling)
            })
            .map(|_| ())
            .map_err(|_| ())
    }
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
        Err(RequestError::Malformed) => {
            let _ = write_response(&mut client, b"HTTP/1.1 400 Bad Request\r\n\r\n");
            return endpoint_event(
                ordinal,
                ResolverExecutionEndpointOutcome::MalformedConnect,
                None,
            );
        }
        Err(RequestError::Oversized) => {
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

enum RequestError {
    Malformed,
    Oversized,
}

fn read_connect_request(
    client: &mut TcpStream,
    stop: &AtomicBool,
) -> Result<ResolverExecutionRequestedEndpoint, RequestError> {
    client
        .set_read_timeout(Some(Duration::from_millis(100)))
        .map_err(|_| RequestError::Malformed)?;
    let deadline = Instant::now() + REQUEST_TIMEOUT;
    let mut request = Vec::with_capacity(512);
    let mut buffer = [0_u8; 512];
    loop {
        if stop.load(Ordering::Acquire) || Instant::now() >= deadline {
            return Err(RequestError::Malformed);
        }
        match client.read(&mut buffer) {
            Ok(0) => return Err(RequestError::Malformed),
            Ok(count) => {
                if request.len().saturating_add(count) > CONNECT_REQUEST_BYTE_LIMIT {
                    return Err(RequestError::Oversized);
                }
                request.extend_from_slice(&buffer[..count]);
                if request.windows(4).any(|window| window == b"\r\n\r\n") {
                    return parse_connect_request(&request);
                }
            }
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) => {}
            Err(_) => return Err(RequestError::Malformed),
        }
    }
}

fn parse_connect_request(
    request: &[u8],
) -> Result<ResolverExecutionRequestedEndpoint, RequestError> {
    let end = request
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or(RequestError::Malformed)?
        + 4;
    if end != request.len() {
        return Err(RequestError::Malformed);
    }
    let request = std::str::from_utf8(request).map_err(|_| RequestError::Malformed)?;
    let lines = request[..request.len() - 4]
        .split("\r\n")
        .collect::<Vec<_>>();
    if lines.is_empty() || lines.len() > CONNECT_HEADER_LIMIT + 1 {
        return Err(if lines.len() > CONNECT_HEADER_LIMIT + 1 {
            RequestError::Oversized
        } else {
            RequestError::Malformed
        });
    }
    let mut request_line = lines[0].split(' ');
    if request_line.next() != Some("CONNECT") {
        return Err(RequestError::Malformed);
    }
    let authority = request_line.next().ok_or(RequestError::Malformed)?;
    if request_line.next() != Some("HTTP/1.1") || request_line.next().is_some() {
        return Err(RequestError::Malformed);
    }
    for header in &lines[1..] {
        let Some((name, value)) = header.split_once(':') else {
            return Err(RequestError::Malformed);
        };
        if name.is_empty()
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
            || value.bytes().any(|byte| byte < b' ' && byte != b'\t')
        {
            return Err(RequestError::Malformed);
        }
        if name.eq_ignore_ascii_case("transfer-encoding")
            || name.eq_ignore_ascii_case("content-length") && value.trim() != "0"
        {
            return Err(RequestError::Malformed);
        }
    }
    parse_authority(authority).map_err(|_| RequestError::Malformed)
}

fn parse_authority(authority: &str) -> io::Result<ResolverExecutionRequestedEndpoint> {
    if authority.starts_with('[') {
        let close = authority
            .find(']')
            .ok_or_else(|| invalid_input("CONNECT authority has an unterminated IPv6 address"))?;
        if authority.as_bytes().get(close + 1) != Some(&b':') {
            return Err(invalid_input("CONNECT authority has no port"));
        }
        let host = &authority[1..close];
        let port = parse_port(&authority[close + 2..])?;
        return ResolverExecutionRequestedEndpoint::new(host, port);
    }
    let (host, port) = authority
        .rsplit_once(':')
        .ok_or_else(|| invalid_input("CONNECT authority has no port"))?;
    if host.contains(':') {
        return Err(invalid_input("CONNECT IPv6 authority is not bracketed"));
    }
    ResolverExecutionRequestedEndpoint::new(host, parse_port(port)?)
}

fn parse_port(port: &str) -> io::Result<u16> {
    if port.is_empty() || !port.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(invalid_input("resolver endpoint port is malformed"));
    }
    let port = port
        .parse::<u16>()
        .map_err(|_| invalid_input("resolver endpoint port is out of range"))?;
    if port == 0 {
        return Err(invalid_input("resolver endpoint port is zero"));
    }
    Ok(port)
}

fn normalize_host(host: &str) -> io::Result<ResolverExecutionEndpointHost> {
    if host.is_empty() || host.len() > 253 || !host.is_ascii() {
        return Err(invalid_input("resolver endpoint host is invalid"));
    }
    if let Ok(address) = IpAddr::from_str(host) {
        return Ok(ResolverExecutionEndpointHost::IpAddress(address));
    }
    let normalized = host.to_ascii_lowercase();
    if normalized.ends_with('.')
        || normalized.split('.').any(|label| {
            label.is_empty()
                || label.len() > 63
                || label.starts_with('-')
                || label.ends_with('-')
                || !label
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
    {
        return Err(invalid_input("resolver endpoint DNS name is invalid"));
    }
    Ok(ResolverExecutionEndpointHost::DnsName(normalized))
}

fn resolve_upstream(endpoint: &ResolverExecutionRequestedEndpoint) -> io::Result<Vec<SocketAddr>> {
    collect_bounded_addresses(endpoint.authority().to_socket_addrs()?)
}

fn collect_bounded_addresses(
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

fn write_response(stream: &mut TcpStream, response: &[u8]) -> io::Result<()> {
    stream.set_write_timeout(Some(Duration::from_secs(1)))?;
    stream.write_all(response)
}

fn relay(
    client: &mut TcpStream,
    upstream: &mut TcpStream,
    stop: &AtomicBool,
    transfer_budget: &ResolverExecutionTransferBudget,
) -> RelayObservation {
    if client.set_nonblocking(true).is_err() || upstream.set_nonblocking(true).is_err() {
        let _ = client.shutdown(Shutdown::Both);
        let _ = upstream.shutdown(Shutdown::Both);
        return RelayObservation::default();
    }
    let deadline = Instant::now() + RELAY_TIMEOUT;
    let mut client_to_upstream = RelayDirection::default();
    let mut upstream_to_client = RelayDirection::default();
    while Instant::now() < deadline && !stop.load(Ordering::Acquire) {
        let mut progressed = false;
        match client_to_upstream.pump(client, upstream, transfer_budget) {
            Ok(direction_progressed) => progressed |= direction_progressed,
            Err(RelayError::TransferLimit) => {
                return relay_observation(&client_to_upstream, &upstream_to_client, true);
            }
            Err(RelayError::Io) => break,
        }
        match upstream_to_client.pump(upstream, client, transfer_budget) {
            Ok(direction_progressed) => progressed |= direction_progressed,
            Err(RelayError::TransferLimit) => {
                return relay_observation(&client_to_upstream, &upstream_to_client, true);
            }
            Err(RelayError::Io) => break,
        }
        if client_to_upstream.complete() && upstream_to_client.complete() {
            return relay_observation(&client_to_upstream, &upstream_to_client, false);
        }
        if !progressed {
            thread::sleep(IO_POLL_INTERVAL);
        }
    }
    let _ = client.shutdown(Shutdown::Both);
    let _ = upstream.shutdown(Shutdown::Both);
    relay_observation(&client_to_upstream, &upstream_to_client, false)
}

#[derive(Default)]
struct RelayObservation {
    uploaded_bytes: u64,
    downloaded_bytes: u64,
    transfer_ceiling_reached: bool,
}

fn relay_observation(
    upload: &RelayDirection,
    download: &RelayDirection,
    transfer_ceiling_reached: bool,
) -> RelayObservation {
    RelayObservation {
        uploaded_bytes: upload.transferred_bytes,
        downloaded_bytes: download.transferred_bytes,
        transfer_ceiling_reached,
    }
}

enum RelayError {
    Io,
    TransferLimit,
}

#[derive(Default)]
struct RelayDirection {
    buffer: Vec<u8>,
    offset: usize,
    source_closed: bool,
    destination_closed: bool,
    transferred_bytes: u64,
}

impl RelayDirection {
    fn pump(
        &mut self,
        source: &mut TcpStream,
        destination: &mut TcpStream,
        transfer_budget: &ResolverExecutionTransferBudget,
    ) -> Result<bool, RelayError> {
        let mut progressed = false;
        if self.offset < self.buffer.len() {
            match destination.write(&self.buffer[self.offset..]) {
                Ok(0) => self.destination_closed = true,
                Ok(count) => {
                    self.offset += count;
                    progressed = true;
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
                Err(_error) => return Err(RelayError::Io),
            }
            if self.offset == self.buffer.len() {
                self.buffer.clear();
                self.offset = 0;
            }
        }
        if self.buffer.is_empty() && !self.source_closed && !self.destination_closed {
            let mut buffer = [0_u8; 16 * 1024];
            match source.read(&mut buffer) {
                Ok(0) => {
                    self.source_closed = true;
                    let _ = destination.shutdown(Shutdown::Write);
                }
                Ok(count) => {
                    transfer_budget
                        .charge(count)
                        .map_err(|()| RelayError::TransferLimit)?;
                    self.transferred_bytes = self
                        .transferred_bytes
                        .checked_add(u64::try_from(count).map_err(|_| RelayError::Io)?)
                        .ok_or(RelayError::Io)?;
                    self.buffer.extend_from_slice(&buffer[..count]);
                    progressed = true;
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
                Err(_error) => return Err(RelayError::Io),
            }
        }
        Ok(progressed)
    }

    fn complete(&self) -> bool {
        (self.source_closed || self.destination_closed) && self.buffer.is_empty()
    }
}

fn encode_socket_address(bytes: &mut Vec<u8>, address: SocketAddr) {
    match address.ip() {
        IpAddr::V4(ip) => {
            bytes.push(1);
            bytes.extend_from_slice(&ip.octets());
        }
        IpAddr::V6(ip) => {
            bytes.push(2);
            bytes.extend_from_slice(&ip.octets());
        }
    }
    bytes.extend_from_slice(&address.port().to_le_bytes());
}

fn encode_bytes(bytes: &mut Vec<u8>, value: &[u8]) {
    bytes.extend_from_slice(&(value.len() as u64).to_le_bytes());
    bytes.extend_from_slice(value);
}

fn invalid_input(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

/// Run the compiler-owned SSH `ProxyCommand` connector.
///
/// The helper accepts no arguments. Its compiler-authored environment names
/// the loopback broker and the already-validated destination. It never opens a
/// socket to the destination itself.
pub fn run_resolver_connect_helper() -> io::Result<()> {
    if std::env::args_os().len() != 1 {
        return Err(invalid_input(
            "the resolver CONNECT helper accepts no arguments",
        ));
    }
    let broker = required_environment(RESOLVER_CONNECT_BROKER_ENVIRONMENT)?
        .parse::<SocketAddr>()
        .map_err(|_| invalid_input("the resolver broker endpoint is malformed"))?;
    if !broker.ip().is_loopback() || broker.port() == 0 {
        return Err(invalid_input(
            "the resolver broker endpoint is not a concrete loopback socket",
        ));
    }
    let target = parse_authority(&required_environment(RESOLVER_CONNECT_TARGET_ENVIRONMENT)?)?;
    let (mut stream, trailing) = open_connect_tunnel(broker, &target)?;
    let mut output = io::stdout().lock();
    if !trailing.is_empty() {
        output.write_all(&trailing)?;
        output.flush()?;
    }
    let mut upload = stream.try_clone()?;
    thread::spawn(move || {
        let _ = io::copy(&mut io::stdin().lock(), &mut upload);
        let _ = upload.shutdown(Shutdown::Write);
    });
    io::copy(&mut stream, &mut output)?;
    output.flush()
}

fn required_environment(name: &str) -> io::Result<String> {
    let value = std::env::var_os(name)
        .ok_or_else(|| invalid_input("the resolver CONNECT helper environment is incomplete"))?;
    value
        .into_string()
        .map_err(|_| invalid_input("the resolver CONNECT helper environment is not UTF-8"))
}

fn open_connect_tunnel(
    broker: SocketAddr,
    target: &ResolverExecutionRequestedEndpoint,
) -> io::Result<(TcpStream, Vec<u8>)> {
    let mut stream = TcpStream::connect_timeout(&broker, REQUEST_TIMEOUT)?;
    stream.set_read_timeout(Some(REQUEST_TIMEOUT))?;
    stream.set_write_timeout(Some(REQUEST_TIMEOUT))?;
    let authority = target.authority();
    write!(
        stream,
        "CONNECT {authority} HTTP/1.1\r\nHost: {authority}\r\nContent-Length: 0\r\n\r\n"
    )?;
    stream.flush()?;

    let trailing = read_connect_response(&mut stream)?;
    stream.set_read_timeout(None)?;
    stream.set_write_timeout(None)?;
    Ok((stream, trailing))
}

fn read_connect_response(stream: &mut TcpStream) -> io::Result<Vec<u8>> {
    let mut response = Vec::with_capacity(512);
    let mut buffer = [0_u8; 512];
    loop {
        let count = stream.read(&mut buffer)?;
        if count == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "the resolver broker closed before its CONNECT response",
            ));
        }
        if response.len().saturating_add(count) > CONNECT_RESPONSE_BYTE_LIMIT {
            return Err(io::Error::other(
                "the resolver broker CONNECT response exceeded its fixed limit",
            ));
        }
        response.extend_from_slice(&buffer[..count]);
        let Some(end) = response.windows(4).position(|window| window == b"\r\n\r\n") else {
            continue;
        };
        let header_end = end + 4;
        let header = std::str::from_utf8(&response[..header_end])
            .map_err(|_| invalid_input("the resolver broker response is not ASCII"))?;
        let mut lines = header[..header.len() - 4].split("\r\n");
        if lines.next() != Some("HTTP/1.1 200 Connection Established")
            || lines.any(|line| {
                line.split_once(':').is_none_or(|(name, value)| {
                    name.is_empty()
                        || !name
                            .bytes()
                            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
                        || value.bytes().any(|byte| byte < b' ' && byte != b'\t')
                })
            })
        {
            return Err(io::Error::other(
                "the resolver broker rejected the CONNECT request",
            ));
        }
        return Ok(response[header_end..].to_vec());
    }
}

#[cfg(test)]
mod tests;
