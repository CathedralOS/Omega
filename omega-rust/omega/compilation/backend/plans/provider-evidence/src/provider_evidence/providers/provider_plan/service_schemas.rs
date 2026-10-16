//! Provider plan and service schema digests, service schemas, methods,
//! progress premises and entry claims.

use super::DigestFeed;
use crate::provider_evidence::evidence_digest;
use crate::provider_evidence::providers::provider_plan::ProviderPlanRow;
use crate::provider_evidence::providers::provider_plan::digest_encoder::ProviderPlanDigestEncoder;
use crate::provider_evidence::providers::provider_plan::identity_table::{
    ProviderIdentityId, ProviderIdentityTable,
};
/// Exact evaluated calling-application identity. This is realization evidence,
/// not a typed-source declaration or permission to execute an application.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoundaryCallingPlanCommitment([u8; 32]);

impl BoundaryCallingPlanCommitment {
    pub const fn from_digest(digest: [u8; 32]) -> Self {
        Self(digest)
    }

    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }

    pub fn is_zero(self) -> bool {
        self.0 == [0; 32]
    }
}

evidence_digest!(
    /// Collision-resistant identity of one exact normalized provider plan.
    ///
    /// Construction remains crate-owned. Compact plan fingerprints are
    /// retained for existing reports and coordinates, but admission joins
    /// should retain this digest or the complete [`ProviderPlan`].
    ProviderPlanDigest,
    from: #[doc(hidden)] pub
);

evidence_digest!(
    /// Collision-resistant identity of one exact normalized service schema.
    ///
    /// This excludes provider realization rows while retaining the
    /// declaration owner, every requirement signature, authority flow, and
    /// selected calling-plan application carried by [`ServiceSchema`].
    ServiceSchemaDigest,
    from: #[doc(hidden)] pub
);

/// The service schema a plan serves: a boundary trait's callable surface,
/// reified from the typed `TraitDefinition` (today that read is scattered
/// -- parameter-count walks in the compiler pipeline, Console detection in
/// the interpreter; the schema type is the one honest carrier).
#[derive(Debug, Clone, Default)]
pub struct ServiceSchema {
    /// The boundary trait's identity (`Console`, `FilesystemHost`) interned
    /// into `identities`; read the canonical text through
    /// [`Self::identity_text`] rather than holding another rendering.
    pub trait_name: ProviderIdentityId,
    /// Interned canonical identities this schema's fields point into.
    /// Projection, not payload: equality and digests join on the interned
    /// text, and mint order is never compared.
    pub identities: ProviderIdentityTable,
    /// Exact package owning the selected boundary trait/operator declaration.
    /// `None` is explicit for toolchain, standalone, or source-free trees.
    pub trait_package_identity: Option<semantic_vocabulary::PackageKeyIdentity>,
    /// Complete applied receiver views owned by this one whole schema. The
    /// first view is the selected root; the remainder are inherited views,
    /// never independently selected provider plans. Empty on non-trait slots
    /// and when a complete application cannot be retained.
    pub receiver_applications: Vec<ServiceReceiverApplication>,
    /// One entry per trait machine, in declaration order.
    pub methods: Vec<ServiceMethod>,
}

/// One exact applied interface and its complete inherited requirement roster.
/// Arguments participate even when they do not occur in a method signature:
/// a phantom `Parent<bool>` must not join a selected `Parent<u64>` view. All
/// identities resolve in the enclosing schema's table, not an external table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceReceiverApplication {
    pub trait_name: ProviderIdentityId,
    pub trait_package_identity: Option<semantic_vocabulary::PackageKeyIdentity>,
    pub argument_identities: Vec<ProviderIdentityId>,
    /// Complete declaration-relative region operands, including phantom ones.
    /// These identities are not runtime carriers and grant no borrowing rights.
    pub lifetime_argument_identities: Vec<ProviderIdentityId>,
    pub requirement_identities: Vec<ProviderIdentityId>,
}

impl ServiceReceiverApplication {
    fn eq_in(
        &self,
        left: &ProviderIdentityTable,
        other: &Self,
        right: &ProviderIdentityTable,
    ) -> bool {
        self.trait_package_identity == other.trait_package_identity
            && left.text(self.trait_name) == right.text(other.trait_name)
            && identity_sequence_eq(
                &self.argument_identities,
                left,
                &other.argument_identities,
                right,
            )
            && identity_sequence_eq(
                &self.lifetime_argument_identities,
                left,
                &other.lifetime_argument_identities,
                right,
            )
            && identity_sequence_eq(
                &self.requirement_identities,
                left,
                &other.requirement_identities,
                right,
            )
    }
}

fn identity_sequence_eq(
    left: &[ProviderIdentityId],
    left_table: &ProviderIdentityTable,
    right: &[ProviderIdentityId],
    right_table: &ProviderIdentityTable,
) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left, right)| left_table.text(*left) == right_table.text(*right))
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ServiceMethod {
    /// Completion premise of this exact requirement, preserved by installation.
    pub completion: semantic_vocabulary::BoundaryCompletion,
    pub name: String,
    /// Semantic owner of the exact requirement. This differs from the
    /// enclosing schema when a target root inherits a stable core requirement
    /// and refines only its calling plan.
    pub requirement_owner: String,
    /// Exact package owning `requirement_owner`. Inherited requirements may
    /// differ from the selected schema owner. `None` is never repaired from
    /// the readable owner or overload identity.
    pub requirement_owner_package_identity: Option<semantic_vocabulary::PackageKeyIdentity>,
    /// Stable named-callable identity of the exact requirement overload,
    /// interned into the schema's `identities`. Provider schemas are never
    /// name-only, including singleton schemas.
    pub requirement_identity: ProviderIdentityId,
    /// Declared parameter count (excluding any receiver) -- the same count
    /// the vtable-field encoder compares against call operands.
    pub parameter_count: usize,
    /// Positional semantic identities of the declared parameter types,
    /// excluding any receiver. Domain qualifications and carry permissions
    /// are part of these identities, so a provider plan cannot be replayed
    /// after an authority-bearing parameter is weakened or replaced.
    /// Indices into the schema's `identities` table.
    pub parameter_type_identities: Vec<ProviderIdentityId>,
    /// Linear routed qualifications accepted at this boundary entry. These
    /// are structured separately from the complete type identity so provider
    /// admission, carry planning, predicate discharge, and authority-flow
    /// artifacts do not have to parse a display string to recover a source
    /// obligation.
    pub entry_claims: Vec<ServiceEntryClaim>,
    /// Whether the method declares a return type.
    pub has_result: bool,
    /// Semantic identity of the declared result type, as an index into the
    /// schema's `identities` table. `ABSENT` denotes no result, not a
    /// unit-shaped result.
    pub result_type_identity: ProviderIdentityId,
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

impl ServiceMethod {
    /// Semantic equality given the tables each method's identities were
    /// interned into: interned fields compare by resolved text, and a
    /// process-local mint order never decides equality.
    fn eq_in(
        &self,
        left_table: &ProviderIdentityTable,
        other: &Self,
        right_table: &ProviderIdentityTable,
    ) -> bool {
        self.completion == other.completion
            && self.name == other.name
            && self.requirement_owner == other.requirement_owner
            && self.requirement_owner_package_identity == other.requirement_owner_package_identity
            && left_table.text(self.requirement_identity)
                == right_table.text(other.requirement_identity)
            && self.parameter_count == other.parameter_count
            && identity_sequence_eq(
                &self.parameter_type_identities,
                left_table,
                &other.parameter_type_identities,
                right_table,
            )
            && self.entry_claims.len() == other.entry_claims.len()
            && self
                .entry_claims
                .iter()
                .zip(&other.entry_claims)
                .all(|(left, right)| {
                    left.parameter_index == right.parameter_index
                        && left_table.text(left.carrier_identity)
                            == right_table.text(right.carrier_identity)
                        && left.domain == right.domain
                        && left.predicate_body == right.predicate_body
                        && left.effective_carry == right.effective_carry
                        && left.authority_flow == right.authority_flow
                })
            && self.has_result == other.has_result
            && left_table.text(self.result_type_identity)
                == right_table.text(other.result_type_identity)
            && self.result_claims == other.result_claims
            && self.service_reach == other.service_reach
            && self.synchronous_invocations == other.synchronous_invocations
            && self.may_suspend == other.may_suspend
            && self.may_block == other.may_block
            && self.terminates_guarantee == other.terminates_guarantee
            && self.termination_premises.len() == other.termination_premises.len()
            && self
                .termination_premises
                .iter()
                .zip(&other.termination_premises)
                .all(|(left, right)| left.eq_in(left_table, right, right_table))
            && self.calling_plan_report_fingerprint == other.calling_plan_report_fingerprint
            && self.calling_plan_commitment == other.calling_plan_commitment
    }
}

impl ServiceProgressPremise {
    /// As [`ServiceMethod::eq_in`], for the nested progress premises.
    fn eq_in(
        &self,
        left_table: &ProviderIdentityTable,
        other: &Self,
        right_table: &ProviderIdentityTable,
    ) -> bool {
        self.profile == other.profile
            && self.subject == other.subject
            && self.subject_projections == other.subject_projections
            && self.establishment_routes.len() == other.establishment_routes.len()
            && self
                .establishment_routes
                .iter()
                .zip(&other.establishment_routes)
                .all(|(left, right)| {
                    left.kind == right.kind
                        && left_table.text(left.requirement_identity)
                            == right_table.text(right.requirement_identity)
                })
    }
}

impl PartialEq for ServiceSchema {
    /// Semantic equality: field values plus the interned text each field
    /// resolves, never the process-local mint order that produced the ids.
    fn eq(&self, other: &Self) -> bool {
        self.trait_package_identity == other.trait_package_identity
            && self.receiver_applications.len() == other.receiver_applications.len()
            && self
                .receiver_applications
                .iter()
                .zip(&other.receiver_applications)
                .all(|(left, right)| left.eq_in(&self.identities, right, &other.identities))
            && self.methods.len() == other.methods.len()
            && self.identity_text(self.trait_name) == other.identity_text(other.trait_name)
            && self
                .methods
                .iter()
                .zip(&other.methods)
                .all(|(left, right)| left.eq_in(&self.identities, right, &other.identities))
    }
}

impl Eq for ServiceSchema {}

impl ServiceSchema {
    /// Check the retained roster's structure, not its source authority. Source
    /// derivation and independent review replay establish the applied edges;
    /// a decoded annotation cannot establish an inheritance relationship.
    pub(crate) fn validate_receiver_applications(&self) -> Result<(), &'static str> {
        let Some(root) = self.receiver_applications.first() else {
            return Ok(());
        };
        if root.trait_package_identity != self.trait_package_identity
            || self.identity_text(root.trait_name) != self.identity_text(self.trait_name)
        {
            return Err("receiver roster root differs from its whole schema owner");
        }
        if root.requirement_identities.len() != self.methods.len()
            || self.methods.iter().any(|method| {
                !root
                    .requirement_identities
                    .contains(&method.requirement_identity)
            })
        {
            return Err("receiver roster root omits or adds an original schema requirement");
        }
        for (position, application) in self.receiver_applications.iter().enumerate() {
            if self.identity_text(application.trait_name).is_empty()
                || application
                    .argument_identities
                    .iter()
                    .chain(&application.lifetime_argument_identities)
                    .any(|identity| self.identity_text(*identity).is_empty())
            {
                return Err("receiver application has an absent or foreign identity");
            }
            if application.requirement_identities.iter().any(|identity| {
                self.identity_text(*identity).is_empty()
                    || !self
                        .methods
                        .iter()
                        .any(|method| method.requirement_identity == *identity)
            }) {
                return Err("receiver application names a requirement outside its whole schema");
            }
            if application
                .requirement_identities
                .windows(2)
                .any(|pair| self.identity_text(pair[0]) >= self.identity_text(pair[1]))
            {
                return Err("receiver requirement roster is duplicated or not canonical");
            }
            if self.receiver_applications[..position]
                .iter()
                .any(|previous| {
                    previous.trait_package_identity == application.trait_package_identity
                        && self.identity_text(previous.trait_name)
                            == self.identity_text(application.trait_name)
                        && identity_sequence_eq(
                            &previous.argument_identities,
                            &self.identities,
                            &application.argument_identities,
                            &self.identities,
                        )
                        && identity_sequence_eq(
                            &previous.lifetime_argument_identities,
                            &self.identities,
                            &application.lifetime_argument_identities,
                            &self.identities,
                        )
                })
            {
                return Err("receiver application is repeated within one whole schema");
            }
        }
        Ok(())
    }

    /// The canonical identity text one of this schema's fields names.
    /// Projects the empty string for the zero id, matching the empty-field
    /// rendering the encoder wrote before interning.
    pub fn identity_text(&self, identity: ProviderIdentityId) -> &str {
        self.identities.text(identity)
    }

    /// Domain-separated SHA-256 commitment to the complete normalized schema.
    pub fn identity_digest(&self) -> ServiceSchemaDigest {
        let mut encoder = ProviderPlanDigestEncoder::for_service_schema();
        encoder.string(self.identity_text(self.trait_name));
        encoder.package_identity(self.trait_package_identity);
        encoder.receiver_applications(self);
        let mut methods = self.methods.iter().collect::<Vec<_>>();
        methods.sort_by(|left, right| {
            left.name.cmp(&right.name).then_with(|| {
                self.identity_text(left.requirement_identity)
                    .cmp(self.identity_text(right.requirement_identity))
            })
        });
        encoder.len(methods.len());
        for method in methods {
            encoder.service_method(method, &self.identities);
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceProgressEstablishmentRoute {
    pub kind: ServiceProgressEstablishmentRouteKind,
    /// Interned into the owning schema's `identities`; route order is fixed
    /// by kind then canonical text so plan digests never see mint order.
    pub requirement_identity: ProviderIdentityId,
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
    /// domain, as an index into the enclosing schema's `identities` table.
    /// Consumers must not recover it by parsing the complete parameter
    /// type or substitute an authored display spelling.
    pub carrier_identity: ProviderIdentityId,
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
    /// Whether this whole schema owns exactly the demanded receiver application
    /// and its entire declared roster. Coverage is never a union of providers.
    pub fn supplies_receiver_schema(&self, demanded: &Self) -> bool {
        self.receiver_application_index(demanded).is_some()
    }

    /// Retain the exact receiver view beneath this whole plan. Its index is
    /// meaningful only together with the digest of the owning provider plan;
    /// consumers can use its complete original roster without rebuilding an
    /// ancestor schema or selecting another provider.
    pub fn receiver_application_index(&self, demanded: &Self) -> Option<usize> {
        let root = demanded.receiver_applications.first()?;
        self.receiver_applications
            .iter()
            .position(|application| self.receiver_contract_matches(application, demanded, root))
    }

    /// Distinct whole choices cannot install the same exact receiver view.
    /// Equal code bodies do not make their selecting owners interchangeable.
    pub fn shares_receiver_application(&self, other: &Self) -> bool {
        self.receiver_applications.iter().any(|left| {
            other
                .receiver_applications
                .iter()
                .any(|right| self.receiver_contract_matches(left, other, right))
        })
    }

    /// An inherited receiver retains the descendant's evaluated calling
    /// contract. Identical declaring requirements under distinct entry plans
    /// do not compete for one installed receiver; equal complete contracts
    /// still cannot mix their choosing owners.
    fn receiver_contract_matches(
        &self,
        application: &ServiceReceiverApplication,
        other: &Self,
        other_application: &ServiceReceiverApplication,
    ) -> bool {
        application.eq_in(&self.identities, other_application, &other.identities)
            && application.requirement_identities.iter().all(|identity| {
                let Some(method) = self
                    .methods
                    .iter()
                    .find(|method| method.requirement_identity == *identity)
                else {
                    return false;
                };
                other.methods.iter().any(|other_method| {
                    method.eq_in(&self.identities, other_method, &other.identities)
                })
            })
    }

    /// Match a provider row to one exact requirement. The readable method name
    /// must agree as a drift check, but only the canonical overload identity
    /// selects the requirement. The row's identity resolves against the owning
    /// plan's `identities` table.
    pub fn row_binds_method(
        &self,
        row: &ProviderPlanRow,
        method: &ServiceMethod,
        row_identities: &ProviderIdentityTable,
    ) -> bool {
        row.requirement_identity.is_valid()
            && row.method == method.name
            && self
                .identities
                .id_at(row_identities.text(row.requirement_identity))
                == Some(method.requirement_identity)
    }

    pub fn method_for_row(
        &self,
        row: &ProviderPlanRow,
        row_identities: &ProviderIdentityTable,
    ) -> Option<&ServiceMethod> {
        self.methods
            .iter()
            .find(|method| self.row_binds_method(row, method, row_identities))
    }
}

#[cfg(test)]
mod tests;
