use std::io;
use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;

const ENDPOINT_OBSERVATION_SCHEMA_VERSION: u32 = 2;
const ENDPOINT_OBSERVATION_CANONICAL_BYTE_LIMIT: usize = 128 * 1024;

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
    pub(super) requested_endpoint: ResolverExecutionRequestedEndpoint,
    pub(super) broker_endpoint: SocketAddr,
    pub(super) connection_limit: u16,
    pub(super) transfer_byte_ceiling: u64,
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
    pub(super) ordinal: u16,
    pub(super) outcome: ResolverExecutionEndpointOutcome,
    pub(super) effective_peer: Option<SocketAddr>,
    pub(super) uploaded_bytes: u64,
    pub(super) downloaded_bytes: u64,
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
    pub(super) route: ResolverExecutionEndpointRoutePolicy,
    pub(super) events: Vec<ResolverExecutionEndpointEvent>,
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

pub(super) fn invalid_input(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}
