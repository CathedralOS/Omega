//! Per-message mediation against the installed generation's private route table.
//! These checks grant request/response delivery, never installation authority.
//!
//! A send is granted only to the request-write end whose *physical token*
//! — the kernel identity of the pipe end itself, attested by the provider
//! from actual custody — equals the recorded assignment. Callers never
//! assert an instance number; the holder is whatever the route table bound
//! to the end that produced the frame. A granted channel operation then
//! delivers through `deliver_*`, which applies the destination contract's
//! exact operation schema on top of the bounded frame envelope and closes
//! the binding on any violation.

use super::frame::{Frame, FrameError};
use super::operation_schema::SchemaViolation;
use super::{ChannelEnd, EndpointId, InstalledTopology, PipeAdapter, RouteRecord};
use std::fmt;

/// A granted request: the mediator delivers the frame to `deliver_to`,
/// which the route table — never the frame — determines.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SendGrant {
    pub binding: u32,
    /// The instance holding the request-read end of `binding`.
    pub deliver_to: u32,
}

/// A granted response: delivered to the importer holding response-read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResponseGrant {
    pub binding: u32,
    /// The instance holding the response-read end of `binding`.
    pub deliver_to: u32,
}

/// Why a channel operation is refused. Each case names the exact failed
/// condition; there is no generic "denied".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvocationRefusal {
    /// The endpoint is not one this installation issued — fabricated,
    /// inherited, or already released ends are all ungranted.
    UngrantedEndpoint { endpoint: EndpointId },
    /// A request arrived on an end that is not request-write, or a response
    /// on an end that is not response-write.
    WrongDirection {
        endpoint: EndpointId,
        role: ChannelEnd,
    },
    /// The physical token attested for the sending end is not the token the
    /// route table recorded — a substituted mapping cannot masquerade as
    /// the assigned end.
    SubstitutedMapping {
        endpoint: EndpointId,
        /// The token recorded at assignment.
        expected: u64,
        /// The token the provider attested for this send.
        presented: u64,
    },
    /// The binding already has an outstanding request.
    RequestInFlight { binding: u32 },
    /// A response arrived with no request outstanding on the binding.
    NoOutstandingRequest { binding: u32 },
    /// The binding was closed by an earlier protocol or peer failure.
    BindingClosed { binding: u32 },
}

impl fmt::Display for InvocationRefusal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UngrantedEndpoint { endpoint } => write!(
                formatter,
                "endpoint {} is not issued by this installation",
                endpoint.0
            ),
            Self::WrongDirection { endpoint, role } => write!(
                formatter,
                "endpoint {} is a {role} end, not a request/response write",
                endpoint.0
            ),
            Self::SubstitutedMapping {
                endpoint,
                expected,
                presented,
            } => write!(
                formatter,
                "endpoint {} records physical token {expected}, not the presented {presented}",
                endpoint.0
            ),
            Self::RequestInFlight { binding } => {
                write!(
                    formatter,
                    "binding {binding} already has a request in flight"
                )
            }
            Self::NoOutstandingRequest { binding } => {
                write!(formatter, "binding {binding} has no outstanding request")
            }
            Self::BindingClosed { binding } => {
                write!(formatter, "binding {binding} is closed")
            }
        }
    }
}

impl std::error::Error for InvocationRefusal {}

/// Why a granted channel operation's frame was refused at delivery. Every
/// refusal has already closed the binding: a peer emitting malformed or
/// schema-violating frames forfeits the channel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeliveryRefusal {
    /// The wire bytes failed the bounded frame envelope — invalid length,
    /// truncation, premature EOF, or trailing bytes.
    Transport { binding: u32, error: FrameError },
    /// The decoded frame violates the destination endpoint's contract
    /// schema — an unlisted operation or a payload outside its law.
    Protocol {
        binding: u32,
        violation: SchemaViolation,
    },
}

impl fmt::Display for DeliveryRefusal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Transport { binding, error } => {
                write!(formatter, "binding {binding} transport failure: {error}")
            }
            Self::Protocol { binding, violation } => {
                write!(formatter, "binding {binding} schema violation: {violation}")
            }
        }
    }
}

impl std::error::Error for DeliveryRefusal {}

impl<A: PipeAdapter, Member> InstalledTopology<A, Member> {
    fn route(&self, endpoint: EndpointId) -> Option<&RouteRecord> {
        self.routes
            .binary_search_by_key(&endpoint, |record| record.id)
            .ok()
            .map(|index| &self.routes[index])
    }

    fn route_for(&self, binding: u32, role: ChannelEnd) -> &RouteRecord {
        self.routes
            .iter()
            .find(|record| record.binding == binding && record.role == role)
            .expect("every binding records all four channel ends")
    }

    fn peer(&self, binding: u32, role: ChannelEnd) -> u32 {
        self.route_for(binding, role).holder
    }

    /// Mediate a request: grant only a request-write end whose physically
    /// attested token equals the recorded assignment, on an open binding
    /// with no outstanding request. The delivery target comes from the
    /// route table — a frame cannot name its peer, and the sender is
    /// whatever the kernel reports holding the end, not a presented index.
    pub fn authorize_send(
        &mut self,
        endpoint: EndpointId,
        presented: u64,
    ) -> Result<SendGrant, InvocationRefusal> {
        let record = self
            .route(endpoint)
            .ok_or(InvocationRefusal::UngrantedEndpoint { endpoint })?;
        if record.role != ChannelEnd::RequestWrite {
            return Err(InvocationRefusal::WrongDirection {
                endpoint,
                role: record.role,
            });
        }
        if record.token != presented {
            return Err(InvocationRefusal::SubstitutedMapping {
                endpoint,
                expected: record.token,
                presented,
            });
        }
        let binding = record.binding;
        if self.closed_bindings.contains(&binding) {
            return Err(InvocationRefusal::BindingClosed { binding });
        }
        if !self.in_flight.insert(binding) {
            return Err(InvocationRefusal::RequestInFlight { binding });
        }
        Ok(SendGrant {
            binding,
            deliver_to: self.peer(binding, ChannelEnd::RequestRead),
        })
    }

    /// Mediate a response: grant only the binding's response-write end whose
    /// attested token matches its assignment, and only while a request is
    /// outstanding. The reply stays on the binding's response channel.
    pub fn authorize_respond(
        &mut self,
        endpoint: EndpointId,
        presented: u64,
    ) -> Result<ResponseGrant, InvocationRefusal> {
        let record = self
            .route(endpoint)
            .ok_or(InvocationRefusal::UngrantedEndpoint { endpoint })?;
        if record.role != ChannelEnd::ResponseWrite {
            return Err(InvocationRefusal::WrongDirection {
                endpoint,
                role: record.role,
            });
        }
        if record.token != presented {
            return Err(InvocationRefusal::SubstitutedMapping {
                endpoint,
                expected: record.token,
                presented,
            });
        }
        let binding = record.binding;
        if self.closed_bindings.contains(&binding) {
            return Err(InvocationRefusal::BindingClosed { binding });
        }
        if !self.in_flight.remove(&binding) {
            return Err(InvocationRefusal::NoOutstandingRequest { binding });
        }
        Ok(ResponseGrant {
            binding,
            deliver_to: self.peer(binding, ChannelEnd::ResponseRead),
        })
    }

    /// Deliver a granted send's frame toward the export end. Undecodable
    /// bytes are a transport failure; a frame the export's contract schema
    /// does not admit is a protocol failure. Either closes the binding —
    /// the grant's `deliver_to` is honored only on `Ok`.
    pub fn deliver_send(
        &mut self,
        grant: SendGrant,
        decoded: Result<&Frame, FrameError>,
    ) -> Result<(), DeliveryRefusal> {
        self.deliver_on(grant.binding, ChannelEnd::RequestRead, decoded)
    }

    /// Deliver a granted response's frame toward the import end, checked
    /// against the demanded contract's operation schema; violations close
    /// the binding exactly as request-side failures do.
    pub fn deliver_respond(
        &mut self,
        grant: ResponseGrant,
        decoded: Result<&Frame, FrameError>,
    ) -> Result<(), DeliveryRefusal> {
        self.deliver_on(grant.binding, ChannelEnd::ResponseRead, decoded)
    }

    /// Check one frame against the destination end's contract schema and
    /// close the binding on failure. `decoded` carries the bounded-envelope
    /// verdict so a transport failure and a schema violation share the one
    /// refusal path.
    fn deliver_on(
        &mut self,
        binding: u32,
        destination: ChannelEnd,
        decoded: Result<&Frame, FrameError>,
    ) -> Result<(), DeliveryRefusal> {
        let frame = match decoded {
            Ok(frame) => frame,
            Err(error) => {
                self.close_binding(binding);
                return Err(DeliveryRefusal::Transport { binding, error });
            }
        };
        let contract = self.route_for(binding, destination).contract;
        match self.schemas.check(&contract, frame) {
            Ok(()) => Ok(()),
            Err(violation) => {
                self.close_binding(binding);
                Err(DeliveryRefusal::Protocol { binding, violation })
            }
        }
    }

    /// Close one binding after a protocol or peer failure: further sends
    /// and responses on it refuse, and no reconnect to another peer is
    /// attempted. A peer process failure surfaces here as EOF on its ends —
    /// the supervisor reports it and the binding closes the same way.
    pub fn close_binding(&mut self, binding: u32) {
        self.in_flight.remove(&binding);
        self.closed_bindings.insert(binding);
    }
}
