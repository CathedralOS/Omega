//! Provider plan and service schema digests, service schemas, methods,
//! progress premises and entry claims.

use crate::effects::capabilities::provider_plan::ProviderPlanRow;
use crate::effects::capabilities::provider_plan::digest_encoder::ProviderPlanDigestEncoder;
use typed_trees::typed_trees::BoundaryCallingPlanCommitment;

/// Collision-resistant identity of one exact normalized provider plan.
///
/// Construction remains crate-owned. Compact plan fingerprints are retained
/// for existing reports and coordinates, but admission joins should retain
/// this digest or the complete [`ProviderPlan`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProviderPlanDigest([u8; 32]);

impl ProviderPlanDigest {
    #[doc(hidden)]
    pub const fn from_digest(digest: [u8; 32]) -> Self {
        Self(digest)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Collision-resistant identity of one exact normalized service schema.
///
/// This excludes provider realization rows while retaining the declaration
/// owner, every requirement signature, authority flow, and selected calling-
/// plan application carried by [`ServiceSchema`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ServiceSchemaDigest([u8; 32]);

impl ServiceSchemaDigest {
    #[doc(hidden)]
    pub const fn from_digest(digest: [u8; 32]) -> Self {
        Self(digest)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// The service schema a plan serves: a boundary trait's callable surface,
/// reified from the typed `TraitDefinition` (today that read is scattered
/// -- parameter-count walks in the compiler pipeline, Console detection in
/// the interpreter; the schema type is the one honest carrier).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ServiceSchema {
    /// The boundary trait's name (`Console`, `FilesystemHost`).
    pub trait_name: String,
    /// Exact package owning the selected boundary trait/operator declaration.
    /// `None` is explicit for toolchain, standalone, or source-free trees.
    pub trait_package_identity: Option<semantic_vocabulary::PackageKeyIdentity>,
    /// One entry per trait machine, in declaration order.
    pub methods: Vec<ServiceMethod>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ServiceMethod {
    pub name: String,
    /// Semantic owner of the exact requirement. This differs from the
    /// enclosing schema when a target root inherits a stable core requirement
    /// and refines only its calling plan.
    pub requirement_owner: String,
    /// Exact package owning `requirement_owner`. Inherited requirements may
    /// differ from the selected schema owner. `None` is never repaired from
    /// the readable owner or overload identity.
    pub requirement_owner_package_identity: Option<semantic_vocabulary::PackageKeyIdentity>,
    /// Stable named-callable identity of the exact requirement overload.
    /// Provider schemas are never name-only, including singleton schemas.
    pub requirement_identity: String,
    /// Declared parameter count (excluding any receiver) -- the same count
    /// the vtable-field encoder compares against call operands.
    pub parameter_count: usize,
    /// Positional semantic identities of the declared parameter types,
    /// excluding any receiver. Domain qualifications and carry permissions
    /// are part of these identities, so a provider plan cannot be replayed
    /// after an authority-bearing parameter is weakened or replaced.
    pub parameter_type_identities: Vec<String>,
    /// Linear routed qualifications accepted at this boundary entry. These
    /// are structured separately from the complete type identity so provider
    /// admission, carry planning, predicate discharge, and authority-flow
    /// artifacts do not have to parse a display string to recover a source
    /// obligation.
    pub entry_claims: Vec<ServiceEntryClaim>,
    /// Whether the method declares a return type.
    pub has_result: bool,
    /// Semantic identity of the declared result type. `None` denotes no
    /// result, not a unit-shaped result.
    pub result_type_identity: Option<String>,
    /// Linear routed qualifications established on this exact result. These
    /// are retained separately from the complete result type so runtime
    /// transition receipts can bind a concrete subject without parsing a
    /// normalized type display.
    pub result_claims: Vec<ServiceResultClaim>,
    /// EFX: normalized boundary-service identities rendered from the
    /// symbol-resolved service table. This includes the containing boundary
    /// trait and any explicit additional reach (with parent closure).
    pub service_reach: Vec<String>,
    /// Direct synchronous boundary bindings this method may enter before it
    /// returns, rendered as the selected binding's boundary-trait identity.
    /// This remains a direct edge set; it is never replaced by reach closure.
    pub synchronous_invocations: Vec<String>,
    /// Independent authored operational ceilings. These never derive from
    /// service reach and participate directly in provider schema identity.
    pub may_suspend: bool,
    pub may_block: bool,
    /// Existing public bodyless requirement guarantee. Progress-profile
    /// premises are retained separately below; private ranking evidence stays
    /// outside provider identity.
    pub terminates_guarantee: bool,
    /// Exact normalized premise schemas. The root distinguishes the installed
    /// provider receiver from caller parameters; projections and profile use
    /// semantic paths.
    pub termination_premises: Vec<ServiceProgressPremise>,
    /// Compact report coordinate for the complete target-closed calling-plan
    /// application selected by a concrete `Calling<C>` relationship. The
    /// application binds the semantic signature, target, selected opaque
    /// representations, and canonical validated `BoundaryEntryPlan`.
    pub calling_plan_report_fingerprint: Option<u64>,
    /// Domain-separated commitment to that exact calling-plan application.
    /// This is present exactly when the report coordinate is present.
    pub calling_plan_commitment: Option<BoundaryCallingPlanCommitment>,
}

impl ServiceSchema {
    /// Domain-separated SHA-256 commitment to the complete normalized schema.
    pub fn identity_digest(&self) -> ServiceSchemaDigest {
        let mut encoder = ProviderPlanDigestEncoder::for_service_schema();
        encoder.string(&self.trait_name);
        encoder.package_identity(self.trait_package_identity);
        let mut methods = self.methods.iter().collect::<Vec<_>>();
        methods.sort_by(|left, right| {
            left.name
                .cmp(&right.name)
                .then_with(|| left.requirement_identity.cmp(&right.requirement_identity))
        });
        encoder.len(methods.len());
        for method in methods {
            encoder.service_method(method);
        }
        ServiceSchemaDigest::from_digest(encoder.finish())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceProgressPremise {
    pub profile: String,
    pub subject: ServiceProgressSubject,
    pub subject_projections: Vec<String>,
    /// Exact owner-authored relationships permitted to establish this
    /// profile. A selected provider plan retains these declarations for
    /// final composition, but none of them is itself an establishment
    /// receipt.
    pub establishment_routes: Vec<ServiceProgressEstablishmentRoute>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ServiceProgressEstablishmentRoute {
    pub kind: ServiceProgressEstablishmentRouteKind,
    pub requirement_identity: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ServiceProgressEstablishmentRouteKind {
    CheckedRequirement,
    BoundaryRequirement,
}

impl ServiceProgressEstablishmentRouteKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CheckedRequirement => "checked_requirement",
            Self::BoundaryRequirement => "boundary_requirement",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ServiceProgressSubject {
    /// The service/provider capability itself; composition supplies the exact
    /// installed provider occurrence.
    ProviderReceiver,
    /// One ordinary caller-visible parameter, excluding the receiver.
    Parameter(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceEntryAuthorityFlow {
    /// The selected provider accepts a caller/external-world claim at entry.
    Accepts,
}

impl ServiceEntryAuthorityFlow {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Accepts => "accepts",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceEntryClaim {
    /// Positional parameter ordinal after excluding any receiver.
    pub parameter_index: usize,
    /// Canonical normalized identity of the carrier qualified by this routed
    /// domain. Consumers must not recover it by parsing the complete parameter
    /// type or substitute an authored display spelling.
    pub carrier_identity: String,
    /// Carrier-aware normalized semantic-domain identity retained by the typed
    /// constraint. This is not the authored short spelling.
    pub domain: String,
    /// Whether the routed qualification also carries predicates that must be
    /// proved at the concrete installed occurrence. Bodyless claims may flow
    /// through the generic external-root acknowledgement path; predicate
    /// claims require a specialized installer that discharges them first.
    pub predicate_body: language_semantics::DomainPredicateBody,
    /// Accepted resource claims are born maximally strict. Exact positive
    /// carry permissions remain separate constrained-type facts.
    pub effective_carry: language_semantics::CarryPolicy,
    pub authority_flow: ServiceEntryAuthorityFlow,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceResultClaim {
    pub domain: String,
    pub effective_carry: language_semantics::CarryPolicy,
}

impl ServiceSchema {
    /// Match a provider row to one exact requirement. The readable method name
    /// must agree as a drift check, but only the canonical overload identity
    /// selects the requirement.
    pub fn row_binds_method(&self, row: &ProviderPlanRow, method: &ServiceMethod) -> bool {
        !row.requirement_identity.is_empty()
            && row.method == method.name
            && row.requirement_identity == method.requirement_identity
    }

    pub fn method_for_row(&self, row: &ProviderPlanRow) -> Option<&ServiceMethod> {
        self.methods
            .iter()
            .find(|method| self.row_binds_method(row, method))
    }
}
