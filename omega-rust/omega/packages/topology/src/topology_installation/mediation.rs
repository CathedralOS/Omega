//! Per-message mediation against the installed generation's private route table.
//! These checks grant request/response delivery, never installation authority.

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
    /// The kernel-attested sender is not the instance this end was assigned
    /// to — a substituted pipe mapping cannot masquerade as the importer.
    SubstitutedMapping {
        endpoint: EndpointId,
        expected: u32,
        actual: u32,
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
                actual,
            } => write!(
                formatter,
                "endpoint {} is assigned to instance {expected}, not sender {actual}",
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

impl<A: PipeAdapter, Member> InstalledTopology<A, Member> {
    fn route(&self, endpoint: EndpointId) -> Option<&RouteRecord> {
        self.routes
            .binary_search_by_key(&endpoint, |record| record.id)
            .ok()
            .map(|index| &self.routes[index])
    }

    fn peer(&self, binding: u32, role: ChannelEnd) -> u32 {
        self.routes
            .iter()
            .find(|record| record.binding == binding && record.role == role)
            .map(|record| record.holder)
            .expect("every binding records all four channel ends")
    }

    /// Mediate a request: grant only a request-write end whose
    /// kernel-attested sender is its assigned holder, on an open binding
    /// with no outstanding request. The delivery target comes from the
    /// route table — a frame cannot name its peer.
    pub fn authorize_send(
        &mut self,
        endpoint: EndpointId,
        sender: u32,
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
        if record.holder != sender {
            return Err(InvocationRefusal::SubstitutedMapping {
                endpoint,
                expected: record.holder,
                actual: sender,
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

    /// Mediate a response: grant only the binding's response-write end held
    /// by the exporter, and only while a request is outstanding. The reply
    /// stays on the binding's response channel.
    pub fn authorize_respond(
        &mut self,
        endpoint: EndpointId,
        sender: u32,
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
        if record.holder != sender {
            return Err(InvocationRefusal::SubstitutedMapping {
                endpoint,
                expected: record.holder,
                actual: sender,
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

    /// Close one binding after a protocol or peer failure: further sends
    /// and responses on it refuse, and no reconnect to another peer is
    /// attempted.
    pub fn close_binding(&mut self, binding: u32) {
        self.in_flight.remove(&binding);
        self.closed_bindings.insert(binding);
    }
}
