use std::io::{self, Read, Write};
use std::net::{IpAddr, Ipv4Addr, Shutdown, SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
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

const ENDPOINT_OBSERVATION_SCHEMA_VERSION: u32 = 1;
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

    pub fn http_proxy_url(&self) -> String {
        format!("http://{}", self.broker_endpoint)
    }

    pub(crate) fn encode(&self, bytes: &mut Vec<u8>) {
        self.requested_endpoint.encode(bytes);
        encode_socket_address(bytes, self.broker_endpoint);
        bytes.extend_from_slice(&self.connection_limit.to_le_bytes());
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
}

impl ResolverExecutionEndpointOutcome {
    const fn tag(self) -> u8 {
        match self {
            Self::Connected => 1,
            Self::MalformedConnect => 2,
            Self::OversizedConnect => 3,
            Self::DestinationRejected => 4,
            Self::UpstreamUnavailable => 5,
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
        }
        assert!(bytes.len() <= ENDPOINT_OBSERVATION_CANONICAL_BYTE_LIMIT);
        bytes
    }
}

#[derive(Debug)]
struct BrokerState {
    stop: AtomicBool,
    events: Mutex<Vec<ResolverExecutionEndpointEvent>>,
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
    pub(crate) fn open(requested_endpoint: ResolverExecutionRequestedEndpoint) -> io::Result<Self> {
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
        };
        let state = Arc::new(BrokerState {
            stop: AtomicBool::new(false),
            events: Mutex::new(Vec::with_capacity(usize::from(ENDPOINT_CONNECTION_LIMIT))),
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
        let _ = relay(&mut client, &mut upstream, stop);
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

fn relay(client: &mut TcpStream, upstream: &mut TcpStream, stop: &AtomicBool) -> io::Result<()> {
    client.set_nonblocking(true)?;
    upstream.set_nonblocking(true)?;
    let deadline = Instant::now() + RELAY_TIMEOUT;
    let mut client_to_upstream = RelayDirection::default();
    let mut upstream_to_client = RelayDirection::default();
    while Instant::now() < deadline && !stop.load(Ordering::Acquire) {
        let mut progressed = false;
        progressed |= client_to_upstream.pump(client, upstream)?;
        progressed |= upstream_to_client.pump(upstream, client)?;
        if client_to_upstream.complete() && upstream_to_client.complete() {
            return Ok(());
        }
        if !progressed {
            thread::sleep(IO_POLL_INTERVAL);
        }
    }
    let _ = client.shutdown(Shutdown::Both);
    let _ = upstream.shutdown(Shutdown::Both);
    Ok(())
}

#[derive(Default)]
struct RelayDirection {
    buffer: Vec<u8>,
    offset: usize,
    source_closed: bool,
    destination_closed: bool,
}

impl RelayDirection {
    fn pump(&mut self, source: &mut TcpStream, destination: &mut TcpStream) -> io::Result<bool> {
        let mut progressed = false;
        if self.offset < self.buffer.len() {
            match destination.write(&self.buffer[self.offset..]) {
                Ok(0) => self.destination_closed = true,
                Ok(count) => {
                    self.offset += count;
                    progressed = true;
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
                Err(error) => return Err(error),
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
                    self.buffer.extend_from_slice(&buffer[..count]);
                    progressed = true;
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
                Err(error) => return Err(error),
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
mod tests {
    use super::*;

    fn send_request(route: &ResolverExecutionEndpointRoute, request: &[u8]) -> Vec<u8> {
        let mut stream = TcpStream::connect(route.policy().broker_endpoint())
            .expect("connect to endpoint broker");
        stream.write_all(request).expect("write CONNECT request");
        stream
            .shutdown(Shutdown::Write)
            .expect("finish CONNECT request");
        let mut response = Vec::new();
        stream
            .read_to_end(&mut response)
            .expect("read proxy response");
        response
    }

    #[test]
    fn requested_endpoint_is_typed_normalized_and_bounded() {
        let endpoint =
            ResolverExecutionRequestedEndpoint::new("GitHub.COM", 443).expect("normalize endpoint");
        assert_eq!(endpoint.authority(), "github.com:443");
        assert!(ResolverExecutionRequestedEndpoint::new("bad_name.example", 443).is_err());
        assert!(ResolverExecutionRequestedEndpoint::new("example.com", 0).is_err());
        assert!(ResolverExecutionRequestedEndpoint::new("2001:db8::1", 22).is_ok());
    }

    #[test]
    fn complete_dns_answer_is_bounded_before_connection_selection() {
        let addresses = (1..=DNS_RESULT_LIMIT + 1).map(|index| {
            SocketAddr::new(
                IpAddr::V4(Ipv4Addr::LOCALHOST),
                u16::try_from(index).expect("test port is bounded"),
            )
        });
        assert!(collect_bounded_addresses(addresses).is_err());
        let exact = (1..=DNS_RESULT_LIMIT).map(|index| {
            SocketAddr::new(
                IpAddr::V4(Ipv4Addr::LOCALHOST),
                u16::try_from(index).expect("test port is bounded"),
            )
        });
        assert_eq!(
            collect_bounded_addresses(exact)
                .expect("exact bounded DNS answer")
                .len(),
            DNS_RESULT_LIMIT
        );
    }

    #[test]
    fn connect_helper_uses_only_the_broker_and_relays_the_tunnel() {
        let upstream = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("bind upstream canary");
        let upstream_endpoint = upstream.local_addr().expect("read upstream endpoint");
        let requested = ResolverExecutionRequestedEndpoint::new(
            &upstream_endpoint.ip().to_string(),
            upstream_endpoint.port(),
        )
        .expect("construct upstream endpoint");
        let route = ResolverExecutionEndpointRoute::open(requested.clone())
            .expect("open compiler endpoint route");
        let upstream_worker = thread::spawn(move || {
            let (mut connection, _) = upstream.accept().expect("accept routed connection");
            let mut request = [0_u8; 4];
            connection
                .read_exact(&mut request)
                .expect("read tunneled request");
            assert_eq!(&request, b"ping");
            connection.write_all(b"pong").expect("write tunneled reply");
        });

        let (mut tunnel, trailing) = open_connect_tunnel(
            route.policy().broker_endpoint(),
            route.policy().requested_endpoint(),
        )
        .expect("open CONNECT tunnel");
        assert!(trailing.is_empty());
        tunnel.write_all(b"ping").expect("write tunnel request");
        tunnel
            .shutdown(Shutdown::Write)
            .expect("finish tunnel request");
        let mut reply = Vec::new();
        tunnel.read_to_end(&mut reply).expect("read tunnel reply");
        assert_eq!(reply, b"pong");

        upstream_worker.join().expect("join upstream canary");
        let observation = route.finish().expect("finish endpoint route");
        assert!(observation.events().iter().any(|event| {
            event.outcome() == ResolverExecutionEndpointOutcome::Connected
                && event.effective_peer() == Some(upstream_endpoint)
        }));
    }

    #[test]
    fn connect_helper_rejects_a_failed_proxy_response() {
        let broker = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("bind rejecting proxy");
        let broker_endpoint = broker.local_addr().expect("read proxy endpoint");
        let worker = thread::spawn(move || {
            let (mut connection, _) = broker.accept().expect("accept helper connection");
            let mut request = [0_u8; 512];
            let _ = connection.read(&mut request).expect("read CONNECT request");
            connection
                .write_all(b"HTTP/1.1 403 Forbidden\r\n\r\n")
                .expect("write rejecting response");
        });
        let requested = ResolverExecutionRequestedEndpoint::new("example.invalid", 22)
            .expect("construct requested endpoint");

        assert!(open_connect_tunnel(broker_endpoint, &requested).is_err());
        worker.join().expect("join rejecting proxy");
    }

    #[test]
    fn broker_rejects_changed_malformed_and_oversized_destinations() {
        let destination = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("bind destination");
        let destination_address = destination.local_addr().expect("destination address");
        let endpoint = ResolverExecutionRequestedEndpoint::new(
            &destination_address.ip().to_string(),
            destination_address.port(),
        )
        .expect("destination endpoint");
        let route = ResolverExecutionEndpointRoute::open(endpoint).expect("open route");

        let changed_port = if destination_address.port() == u16::MAX {
            destination_address.port() - 1
        } else {
            destination_address.port() + 1
        };
        let changed = send_request(
            &route,
            format!("CONNECT 127.0.0.1:{changed_port} HTTP/1.1\r\n\r\n").as_bytes(),
        );
        assert!(changed.starts_with(b"HTTP/1.1 403"));
        let malformed = send_request(&route, b"GET / HTTP/1.1\r\n\r\n");
        assert!(malformed.starts_with(b"HTTP/1.1 400"));
        let mut oversized = b"CONNECT 127.0.0.1:1 HTTP/1.1\r\nX: ".to_vec();
        oversized.resize(CONNECT_REQUEST_BYTE_LIMIT + 1, b'a');
        let response = send_request(&route, &oversized);
        assert!(response.starts_with(b"HTTP/1.1 431"));

        let observation = route.finish().expect("finish route");
        assert_eq!(observation.events().len(), 3);
        assert_eq!(
            observation.events()[0].outcome(),
            ResolverExecutionEndpointOutcome::DestinationRejected
        );
        assert_eq!(
            observation.events()[1].outcome(),
            ResolverExecutionEndpointOutcome::MalformedConnect
        );
        assert_eq!(
            observation.events()[2].outcome(),
            ResolverExecutionEndpointOutcome::OversizedConnect
        );
        assert!(
            observation
                .events()
                .iter()
                .all(|event| event.effective_peer().is_none())
        );
    }

    #[test]
    fn broker_records_the_effective_connected_peer() {
        let destination = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("bind destination");
        let destination_address = destination.local_addr().expect("destination address");
        let acceptance = thread::spawn(move || {
            let (mut stream, _) = destination.accept().expect("accept broker connection");
            let mut byte = [0_u8; 1];
            stream.read_exact(&mut byte).expect("read relayed byte");
            stream.write_all(&byte).expect("echo relayed byte");
        });
        let endpoint = ResolverExecutionRequestedEndpoint::new(
            &destination_address.ip().to_string(),
            destination_address.port(),
        )
        .expect("destination endpoint");
        let route = ResolverExecutionEndpointRoute::open(endpoint.clone()).expect("open route");
        let mut client =
            TcpStream::connect(route.policy().broker_endpoint()).expect("connect proxy");
        write!(
            client,
            "CONNECT {} HTTP/1.1\r\nHost: {}\r\n\r\n",
            endpoint.authority(),
            endpoint.authority()
        )
        .expect("write CONNECT");
        let mut response = [0_u8; 39];
        client
            .read_exact(&mut response)
            .expect("read CONNECT response");
        assert_eq!(&response, b"HTTP/1.1 200 Connection Established\r\n\r\n");
        client.write_all(b"x").expect("write tunneled byte");
        let mut echoed = [0_u8; 1];
        client.read_exact(&mut echoed).expect("read tunneled byte");
        assert_eq!(echoed, *b"x");
        drop(client);
        acceptance.join().expect("destination worker");

        let observation = route.finish().expect("finish route");
        assert_eq!(observation.events().len(), 1);
        assert_eq!(
            observation.events()[0].outcome(),
            ResolverExecutionEndpointOutcome::Connected
        );
        assert_eq!(
            observation.events()[0].effective_peer(),
            Some(destination_address)
        );
        assert_eq!(observation.route().requested_endpoint(), &endpoint);
        assert_eq!(
            observation.canonical_bytes(),
            observation.clone().canonical_bytes()
        );
    }

    #[test]
    fn broker_acceptance_and_observation_are_connection_bounded() {
        let endpoint =
            ResolverExecutionRequestedEndpoint::new("127.0.0.1", 9).expect("discard endpoint");
        let route = ResolverExecutionEndpointRoute::open(endpoint).expect("open route");
        for _ in 0..ENDPOINT_CONNECTION_LIMIT {
            let response = send_request(&route, b"GET / HTTP/1.1\r\n\r\n");
            assert!(response.starts_with(b"HTTP/1.1 400"));
        }
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            match TcpStream::connect(route.policy().broker_endpoint()) {
                Err(_) => break,
                Ok(stream) => drop(stream),
            }
            assert!(
                Instant::now() < deadline,
                "broker retained an unbounded accept surface"
            );
            thread::sleep(IO_POLL_INTERVAL);
        }
        let observation = route.finish().expect("finish route");
        assert_eq!(
            observation.events().len(),
            usize::from(ENDPOINT_CONNECTION_LIMIT)
        );
        assert_eq!(
            observation.events().last().expect("last event").ordinal(),
            ENDPOINT_CONNECTION_LIMIT
        );
    }
}
