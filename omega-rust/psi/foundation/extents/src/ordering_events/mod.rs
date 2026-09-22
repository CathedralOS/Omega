//! Device ordering-role events: the discriminant-bound records a device
//! boundary exchanges once exact extent authority is admitted.
//!
//! Publication, cache maintenance, notification, completion, and
//! acquisition are distinct protocol roles; the
//! [`DeviceOrderingRole`] discriminant participates in canonical event
//! identity, so equal payloads under different roles are never
//! interchangeable. Each event binds its exact coordinate ranges and
//! mappings, a stable device instance, and a runtime queue/session scope —
//! provisional structural constructors are absent on purpose, since a bare
//! row carries no event, Stable-view, completion, or lowering authority.

use std::collections::{BTreeMap, BTreeSet};

use crate::extent::diagnostic::{ExtentDiagnostic, validate_range};
use crate::external_loans::ExternalLoanId;
use crate::identities::{
    AddressSpaceId, ExtentLineageId, ExtentProvenanceId, MappingEraId,
    normalized_extent_identity,
};
use crate::mapping::MappingId;

/// The protocol role an ordering event performs — part of the event's
/// canonical identity, never a payload decoration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DeviceOrderingRole {
    /// Names the place and current write state a notification may later
    /// consume; an intersecting write invalidates it first.
    Publication,
    /// A scoped target cache-maintenance or barrier realization between
    /// publication and device observation.
    CacheMaintenance,
    /// The posted-write observation a device may make once publication and
    /// any required maintenance stand.
    Notification,
    /// Device completion evidence tied to one request — a CPU barrier alone
    /// cannot establish it on every fabric.
    Completion,
    /// Consumes the completion tied to the same request, loan, device, and
    /// runtime scope; only proven release restores a Stable CPU view.
    Acquisition,
}

impl DeviceOrderingRole {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Publication => "publication",
            Self::CacheMaintenance => "cache-maintenance",
            Self::Notification => "notification",
            Self::Completion => "completion",
            Self::Acquisition => "acquisition",
        }
    }
}

normalized_extent_identity!(OrderingEventId, "ordering-event");

normalized_extent_identity!(
    /// The stable device instance an ordering event names — distinct from the
    /// per-transfer borrower identity an external loan carries.
    DeviceInstanceId,
    "device-instance"
);

normalized_extent_identity!(
    /// The runtime queue or session scope an ordering event binds.
    RuntimeScopeId,
    "runtime-scope"
);

/// Which coordinate family one bound range orders — roles may relate
/// different coordinate kinds, so a uniform one-range carrier is not the
/// public shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OrderingCoordinateKind {
    Data,
    Descriptor,
    Doorbell,
    ReadBack,
    Request,
    Completion,
}

/// One coordinate's exact range under the mapping that realizes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OrderingRange {
    pub mapping: MappingId,
    pub base: u64,
    pub length: u64,
}

/// The extent authority an ordering event binds — provenance, mapping era,
/// and lineage pin an identical coordinate range to the exact authority
/// that was lent, the same binding an external reach receipt makes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OrderingAuthority {
    pub address_space: AddressSpaceId,
    pub provenance: ExtentProvenanceId,
    pub era: MappingEraId,
    pub lineage: ExtentLineageId,
}

/// A provider-issued ordering event: role, device, runtime scope, request,
/// authority, and the exact coordinate ranges it orders.
#[derive(Debug, PartialEq, Eq)]
pub struct OrderingEvent {
    identity: OrderingEventId,
    role: DeviceOrderingRole,
    device: DeviceInstanceId,
    scope: RuntimeScopeId,
    request: ExternalLoanId,
    authority: OrderingAuthority,
    coordinates: BTreeMap<OrderingCoordinateKind, OrderingRange>,
}

impl OrderingEvent {
    /// Bind an ordering event to admitted authority. The provider restates
    /// the request, device, scope, and extent authority it was admitted
    /// under; construction here establishes the event only — no Stable
    /// view, completion, or lowering authority is implied.
    pub fn from_admitted_provider(
        identity: OrderingEventId,
        role: DeviceOrderingRole,
        device: DeviceInstanceId,
        scope: RuntimeScopeId,
        request: ExternalLoanId,
        authority: OrderingAuthority,
        coordinates: impl IntoIterator<Item = (OrderingCoordinateKind, OrderingRange)>,
    ) -> Result<Self, ExtentDiagnostic> {
        let mut bound = BTreeMap::new();
        for (kind, range) in coordinates {
            validate_range(range.base, range.length)?;
            if bound.insert(kind, range).is_some() {
                return Err(ExtentDiagnostic(format!(
                    "ordering event carries duplicate {:?} coordinate",
                    kind
                )));
            }
        }
        if bound.is_empty() {
            return Err(ExtentDiagnostic(
                "ordering event must bind at least one coordinate range".into(),
            ));
        }
        Ok(Self {
            identity,
            role,
            device,
            scope,
            request,
            authority,
            coordinates: bound,
        })
    }

    pub const fn identity(&self) -> OrderingEventId {
        self.identity
    }

    pub const fn role(&self) -> DeviceOrderingRole {
        self.role
    }

    pub const fn device(&self) -> DeviceInstanceId {
        self.device
    }

    pub const fn scope(&self) -> RuntimeScopeId {
        self.scope
    }

    pub const fn request(&self) -> ExternalLoanId {
        self.request
    }

    pub const fn authority(&self) -> OrderingAuthority {
        self.authority
    }

    pub fn coordinate(&self, kind: OrderingCoordinateKind) -> Option<OrderingRange> {
        self.coordinates.get(&kind).copied()
    }

    /// Whether two events carry the same payload — every bound field —
    /// under different roles. Equality of the records themselves is the
    /// identity rule; this reads the payload-sharing case the role
    /// discriminant must keep distinct.
    pub fn same_payload_different_role(&self, other: &Self) -> bool {
        self.role != other.role
            && self.device == other.device
            && self.scope == other.scope
            && self.request == other.request
            && self.authority == other.authority
            && self.coordinates == other.coordinates
    }
}

/// What a coverage admission requires: the device, runtime scope, request,
/// and authority every event must share, plus the role roster the boundary
/// declares complete.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderingCoverageRequirement {
    pub device: DeviceInstanceId,
    pub scope: RuntimeScopeId,
    pub request: ExternalLoanId,
    pub authority: OrderingAuthority,
    pub roles: BTreeSet<DeviceOrderingRole>,
}

/// The admitted roster: one event per required role, all bound to the
/// requirement's device, scope, request, and authority.
#[derive(Debug, PartialEq, Eq)]
pub struct OrderingCoverageAdmission {
    events: Vec<OrderingEvent>,
}

impl OrderingCoverageAdmission {
    pub fn events(&self) -> &[OrderingEvent] {
        &self.events
    }
}

/// A rejected coverage roster. The provided events are handed back —
/// rejection consumes no retry custody.
#[derive(Debug)]
pub struct OrderingCoverageRejection {
    events: Vec<OrderingEvent>,
    diagnostic: ExtentDiagnostic,
}

impl OrderingCoverageRejection {
    pub const fn diagnostic(&self) -> &ExtentDiagnostic {
        &self.diagnostic
    }

    pub fn into_events(self) -> Vec<OrderingEvent> {
        self.events
    }
}

/// Admit a provider's declared ordering roster against the required role
/// coverage. Missing, extra, duplicate, or drifted coverage rejects and
/// returns the provided events — the rejection consumes no retry custody.
pub fn admit_role_coverage(
    requirement: &OrderingCoverageRequirement,
    events: Vec<OrderingEvent>,
) -> Result<OrderingCoverageAdmission, OrderingCoverageRejection> {
    let reject = |events: Vec<OrderingEvent>, message: String| {
        Err(OrderingCoverageRejection {
            events,
            diagnostic: ExtentDiagnostic(message),
        })
    };

    let mut covered: BTreeSet<DeviceOrderingRole> = BTreeSet::new();
    let mut failure: Option<String> = None;
    for event in &events {
        if event.device != requirement.device {
            failure = Some("ordering event names a different stable device instance".into());
        } else if event.scope != requirement.scope {
            failure = Some("ordering event names a different runtime queue/session scope".into());
        } else if event.request != requirement.request {
            failure = Some("ordering event names a different external-loan request".into());
        } else if event.authority != requirement.authority {
            failure = Some("ordering event binds drifted extent authority".into());
        } else if !requirement.roles.contains(&event.role) {
            failure = Some(format!(
                "ordering event carries extra {:?} coverage",
                event.role
            ));
        } else if !covered.insert(event.role) {
            failure = Some(format!(
                "ordering event duplicates {:?} coverage",
                event.role
            ));
        }
        if failure.is_some() {
            break;
        }
    }
    if let Some(message) = failure {
        return reject(events, message);
    }
    if covered != requirement.roles {
        let missing: Vec<&'static str> = requirement
            .roles
            .difference(&covered)
            .map(|role| role.name())
            .collect();
        return reject(
            events,
            format!(
                "ordering coverage is missing {} role(s)",
                missing.join(", ")
            ),
        );
    }
    Ok(OrderingCoverageAdmission { events })
}

/// Whether one acquisition event consumes the completion tied to the same
/// request, device, runtime scope, and extent authority — the only route
/// that restores a Stable CPU view.
pub fn admit_acquisition_after_completion(
    acquisition: &OrderingEvent,
    completion: &OrderingEvent,
) -> Result<(), ExtentDiagnostic> {
    let mismatch = if acquisition.role != DeviceOrderingRole::Acquisition {
        Some("event is not an acquisition")
    } else if completion.role != DeviceOrderingRole::Completion {
        Some("companion event is not a completion")
    } else if acquisition.request != completion.request {
        Some("acquisition names a different external-loan request")
    } else if acquisition.device != completion.device {
        Some("acquisition names a different stable device instance")
    } else if acquisition.scope != completion.scope {
        Some("acquisition names a different runtime queue/session scope")
    } else if acquisition.authority != completion.authority {
        Some("acquisition binds different extent authority")
    } else {
        None
    };
    mismatch.map_or(Ok(()), |message| Err(ExtentDiagnostic(message.into())))
}

#[cfg(test)]
mod tests;
