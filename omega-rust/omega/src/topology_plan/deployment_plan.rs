//! Exact plan and owner-request data.
//!
//! Every record here is canonical: collections are stored sorted by their
//! documented keys so that encoding is byte-exact and the plan subject and
//! request commitment (digests over canonical bytes) are stable. Constructors
//! normalize; the codec additionally *requires* canonical order on the wire so
//! one logical value has exactly one byte identity.
//!
//! `InstanceKey` is the authored instance name (unique per plan);
//! `EndpointKey = (instance index, slot)`. The plan subject never occurs
//! inside its own hashed records — `plan_subject` is computed over encoded
//! bytes, not stored in them.

pub mod codec;
pub mod graph;
pub mod predicate;

use sha2::{Digest, Sha256};
use std::fmt;

/// A 32-byte exact identity: the digest of the referenced record.
/// Zero is reserved for "no identity" and is never a valid referent.
pub type Identity = [u8; 32];

/// Compute the canonical identity of encoded bytes.
pub fn identity_of(bytes: &[u8]) -> Identity {
    let digest = Sha256::digest(bytes);
    let mut identity = [0u8; 32];
    identity.copy_from_slice(&digest);
    identity
}

/// The plan's exact subject: the digest of its complete canonical records.
/// External references use `(plan subject, local key)`.
pub fn plan_subject(plan_bytes: &[u8]) -> Identity {
    identity_of(plan_bytes)
}

/// The request's exact commitment: the digest of its canonical bytes. The
/// installer receives this commitment independently of any candidate plan.
pub fn request_commitment(request_bytes: &[u8]) -> Identity {
    identity_of(request_bytes)
}

/// Unique authored instance name — the plan-local `InstanceKey`.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InstanceName(String);

impl InstanceName {
    pub fn new(name: impl Into<String>) -> Result<Self, NameError> {
        let name = name.into();
        if name.is_empty() {
            return Err(NameError::Empty);
        }
        if name.len() > crate::topology_plan::deployment_plan::codec::MAX_NAME_BYTES {
            return Err(NameError::TooLong);
        }
        Ok(Self(name))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for InstanceName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "InstanceName({:?})", self.0)
    }
}

impl fmt::Display for InstanceName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameError {
    Empty,
    TooLong,
}

impl fmt::Display for NameError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("instance name must not be empty"),
            Self::TooLong => formatter.write_str("instance name exceeds the bounded length"),
        }
    }
}

impl std::error::Error for NameError {}

/// Direction of one public endpoint within a component interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EndpointDirection {
    /// The instance demands a binding to a peer's export.
    Import,
    /// The instance offers this endpoint for peers' imports.
    Export,
}

/// One public endpoint in a component interface. `(slot, direction)` is unique
/// within an instance — an artifact advertising two endpoints with the same
/// label/direction rejects at normalization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoint {
    /// Nominal slot identity within the interface.
    pub slot: u32,
    pub direction: EndpointDirection,
    /// Exact requirement-application identity (contract + schema versions).
    pub contract: Identity,
}

/// Verifier-established completeness of a component description. Only the
/// verifier's closed communication-authority profile is representable here;
/// producer-declared or early-frontier tags are named codec rejections, never
/// constructible values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Completeness {
    /// Complete entries, outgoing authority, and custody constraints were
    /// verified; `closure` binds that exact evidence.
    VerifiedComplete { closure: Identity },
}

/// The component facts a plan consumes, identified exactly. Construction data
/// (endpoint inventory, demanded imports) lives beside it on `PlanInstance`;
/// the description identity binds which verified description supplied them.
/// A producer-written "complete" flag cannot appear here — see `Completeness`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentDescription {
    /// Exact component semantic subject.
    pub subject: Identity,
    /// Identity of the verification profile that established the facts.
    pub verification_profile: Identity,
    pub completeness: Completeness,
    /// Assumptions the description requires; each must be accepted by the
    /// owner request. Canonical (sorted, unique) order.
    pub assumptions: Vec<Identity>,
}

/// Whether an instance is a checked component or an explicitly modeled
/// external participant. v1 fixes the roster; wildcard/dynamic participants
/// are not representable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstanceRole {
    Component,
    ExternalParticipant,
}

/// One roster member: its key, exact component description, role, and public
/// endpoint inventory as consumed from the verified description.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanInstance {
    pub name: InstanceName,
    pub component: ComponentDescription,
    pub role: InstanceRole,
    /// Public endpoint inventory in canonical `(slot, direction)` order.
    /// Every `Import` endpoint is a demanded import and must have exactly one
    /// binding.
    pub endpoints: Vec<Endpoint>,
}

/// `(InstanceKey, slot)` — the canonical endpoint identity inside a plan.
/// `instance` indexes the canonical instance roster.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct EndpointKey {
    pub instance: u32,
    pub slot: u32,
}

/// One directed binding: an instance's import to a peer's export, with the
/// selected transport realization. Multiple endpoint edges may join the same
/// instance pair; every demanded import binds exactly once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binding {
    /// Must resolve to an `Import` endpoint.
    pub import: EndpointKey,
    /// Must resolve to an `Export` endpoint.
    pub export: EndpointKey,
    /// Exact selected transport/codec realization identity; must be a member
    /// of the request's allowed transport profile.
    pub transport: Identity,
}

/// An explicit instance set in canonical (sorted, unique) order. Empty sets,
/// unknown members, duplicates, and foreign-plan handles reject at selector
/// validation rather than satisfying a routing property vacuously.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicySelector {
    pub members: Vec<InstanceName>,
}

impl PolicySelector {
    pub fn new(members: impl IntoIterator<Item = InstanceName>) -> Self {
        let mut members: Vec<InstanceName> = members.into_iter().collect();
        members.sort();
        Self { members }
    }
}

/// The fixed v1 predicate set supplied by the selected reference verifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PolicyPredicate {
    /// No directed finite path from any instance in `sources` to any in
    /// `targets`. Deliberately permits disconnection.
    NoRoute,
    /// At least one source-to-target path exists, and every such path visits
    /// a `via` instance.
    OnlyVia,
}

/// One required policy: predicate identity plus exact instance-set arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyCall {
    pub predicate: PolicyPredicate,
    pub sources: PolicySelector,
    pub targets: PolicySelector,
    /// Empty for `NoRoute`; required non-empty for `OnlyVia`.
    pub via: PolicySelector,
}

impl PolicyCall {
    pub fn no_route(sources: PolicySelector, targets: PolicySelector) -> Self {
        Self {
            predicate: PolicyPredicate::NoRoute,
            sources,
            targets,
            via: PolicySelector {
                members: Vec::new(),
            },
        }
    }

    pub fn only_via(sources: PolicySelector, targets: PolicySelector, via: PolicySelector) -> Self {
        Self {
            predicate: PolicyPredicate::OnlyVia,
            sources,
            targets,
            via,
        }
    }

    /// Canonical ordering key: predicate tag, then argument members in order.
    pub fn canonical_key(
        &self,
    ) -> (
        PolicyPredicate,
        Vec<InstanceName>,
        Vec<InstanceName>,
        Vec<InstanceName>,
    ) {
        (
            self.predicate,
            self.sources.members.clone(),
            self.targets.members.clone(),
            self.via.members.clone(),
        )
    }
}

/// Witness path as instance indices in traversal order (first ∈ sources,
/// last ∈ targets, consecutive edges exist in the relevant graph).
pub type WitnessPath = Vec<u32>;

/// Why a policy evaluated `Violated`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Violation {
    /// A concrete path defeating the policy: any `sources`→`targets` path for
    /// `no_route`, or one avoiding `via` for `only_via`. Shortest, with
    /// canonical-key tie breaking.
    Bypass { path: WitnessPath },
    /// `only_via` only: no source reaches any target at all, so the
    /// non-vacuity premise fails.
    Disconnected,
}

/// Independently checkable evidence for a `Satisfied` outcome. The verifier
/// does not trust the producer's traversal: each certificate states a
/// decidable property that proves the predicate directly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Certificate {
    /// `no_route` evidence: a set containing `sources`, closed under every
    /// graph edge, disjoint from `targets`. Closure establishes absence of
    /// any source-to-target path by induction on path length. Sorted indices.
    NoRoute { reachable: Vec<u32> },
    /// `only_via` evidence: `path` is one valid source-to-target witness
    /// (the non-vacuity leg); `reachable` contains `sources`, is closed under
    /// edges that avoid `via`, and is disjoint from `targets` — so any path
    /// avoiding `via` would lie inside it and cannot reach `targets`.
    OnlyVia {
        path: WitnessPath,
        reachable: Vec<u32>,
    },
}

/// The recorded result of executing one required policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyOutcome {
    Satisfied {
        certificate: Certificate,
    },
    /// Decodable so a corrupt or unfinished composition is rejected on
    /// inspection; never valid inside an admitted plan.
    Violated {
        violation: Violation,
    },
}

/// One executed policy row: the exact policy executable, its arguments, and
/// the recorded outcome with checkable evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutedPolicy {
    pub call: PolicyCall,
    /// Identity of the policy executable that produced this evidence. Must
    /// equal the request's selected verifier; compared by identity, never
    /// loaded.
    pub verifier: Identity,
    pub outcome: PolicyOutcome,
}

/// The frozen deployment plan: the request it answers, the instance roster
/// with endpoint inventories, every binding, and each required policy's
/// recorded outcome. Canonical order throughout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeploymentPlan {
    /// Commitment of the request this plan was composed for.
    pub request_commitment: Identity,
    /// Frozen instance roster, sorted by name.
    pub instances: Vec<PlanInstance>,
    /// All endpoint bindings, sorted by `(import, export)` keys.
    pub bindings: Vec<Binding>,
    /// Recorded policy rows, sorted by `PolicyCall::canonical_key`.
    pub policies: Vec<ExecutedPolicy>,
}

/// Owner intent, supplied independently of any candidate plan. v1 contents:
/// the exact instance roster and component subjects, required policies and
/// arguments, the selected verifier identity, the allowed transport profile,
/// and accepted assumptions. Updates need fresh owner authorization; there is
/// no compatibility heuristic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopologyRequest {
    /// Exact roster: name plus component subject. Sorted by name.
    pub instances: Vec<RequestedInstance>,
    /// Required policy set, sorted by `PolicyCall::canonical_key`.
    pub policies: Vec<PolicyCall>,
    /// The one selected policy-verifier identity for v1.
    pub verifier: Identity,
    /// Allowed transport realizations a binding may select. Canonical order.
    pub transports: Vec<Identity>,
    /// Assumptions the owner accepts for this composition. Canonical order.
    pub accepted_assumptions: Vec<Identity>,
}

/// One roster member as the owner requires it: an exact instance name bound
/// to an exact component subject.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestedInstance {
    pub name: InstanceName,
    pub subject: Identity,
}

#[cfg(test)]
mod tests {
    use super::{InstanceName, NameError, PolicySelector};

    #[test]
    fn instance_name_rejects_empty_and_overlong() {
        assert_eq!(InstanceName::new(""), Err(NameError::Empty));
        let overlong = "x".repeat(crate::topology_plan::deployment_plan::codec::MAX_NAME_BYTES + 1);
        assert_eq!(InstanceName::new(overlong), Err(NameError::TooLong));
    }

    #[test]
    fn selectors_are_canonicalized() {
        let selector = PolicySelector::new([
            InstanceName::new("billing").unwrap(),
            InstanceName::new("api").unwrap(),
        ]);
        assert_eq!(selector.members[0].as_str(), "api");
        assert_eq!(selector.members[1].as_str(), "billing");
    }
}
