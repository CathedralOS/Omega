use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageReviewDomainClassification {
    ProgressProfile,
}

/// One compiler-owned semantic role contributed by a public domain.
///
/// The role vocabulary is closed compiler semantics. The declaration's
/// compiler-private semantic-domain ID is validated during projection but does
/// not cross the canonical package-review boundary; the package-qualified
/// domain identity is the persistent subject.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewDomainSemanticRole {
    DenotationDimension,
    ArithmeticPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewDomainEstablishmentKind {
    CheckedRequirement,
    BoundaryRequirement,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewDomainEstablishmentRoute {
    pub(crate) kind: PackageReviewDomainEstablishmentKind,
    pub(crate) trait_identity: PackageReviewNominalIdentity,
    pub(crate) requirement_identity: PackageReviewNominalIdentity,
}

impl PackageReviewDomainEstablishmentRoute {
    pub const fn kind(&self) -> PackageReviewDomainEstablishmentKind {
        self.kind
    }

    pub const fn trait_identity(&self) -> &PackageReviewNominalIdentity {
        &self.trait_identity
    }

    pub const fn requirement_identity(&self) -> &PackageReviewNominalIdentity {
        &self.requirement_identity
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewDomainAliasAtom {
    Declared(PackageReviewNominalIdentity),
    Carry(psi_language_semantics::CarryPermission),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageReviewDomainShape {
    pub(crate) identity: PackageReviewNominalIdentity,
    pub(crate) type_parameters: Vec<PackageReviewTypeParameter>,
    pub(crate) target_type: PackageReviewTypeIdentity,
    pub(crate) index_arguments: Vec<PackageReviewTypeIdentity>,
    pub(crate) predicate_body: psi_language_semantics::DomainPredicateBody,
    pub(crate) predicate_facts: Vec<PackageReviewContractFact>,
    pub(crate) alias_expansion: Option<Vec<PackageReviewDomainAliasAtom>>,
    pub(crate) classification: Option<PackageReviewDomainClassification>,
    pub(crate) semantic_roles: Vec<PackageReviewDomainSemanticRole>,
    pub(crate) establishment_routes: Vec<PackageReviewDomainEstablishmentRoute>,
}

impl PackageReviewDomainShape {
    pub const fn identity(&self) -> &PackageReviewNominalIdentity {
        &self.identity
    }

    pub fn type_parameters(&self) -> &[PackageReviewTypeParameter] {
        &self.type_parameters
    }

    pub const fn target_type(&self) -> &PackageReviewTypeIdentity {
        &self.target_type
    }

    pub fn index_arguments(&self) -> &[PackageReviewTypeIdentity] {
        &self.index_arguments
    }

    pub const fn predicate_body(&self) -> psi_language_semantics::DomainPredicateBody {
        self.predicate_body
    }

    pub fn predicate_facts(&self) -> &[PackageReviewContractFact] {
        &self.predicate_facts
    }

    pub fn alias_expansion(&self) -> Option<&[PackageReviewDomainAliasAtom]> {
        self.alias_expansion.as_deref()
    }

    pub const fn classification(&self) -> Option<PackageReviewDomainClassification> {
        self.classification
    }

    pub fn semantic_roles(&self) -> &[PackageReviewDomainSemanticRole] {
        &self.semantic_roles
    }

    pub fn establishment_routes(&self) -> &[PackageReviewDomainEstablishmentRoute] {
        &self.establishment_routes
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageReviewDataField {
    pub(crate) identity: Option<u64>,
    pub(crate) name: String,
    pub(crate) relevance: psi_language_core::BindingRelevance,
    pub(crate) type_identity: PackageReviewTypeIdentity,
}

impl PackageReviewDataField {
    pub const fn identity(&self) -> Option<u64> {
        self.identity
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn relevance(&self) -> psi_language_core::BindingRelevance {
        self.relevance
    }

    pub const fn type_identity(&self) -> &PackageReviewTypeIdentity {
        &self.type_identity
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageReviewDataMember {
    Field(PackageReviewDataField),
    Variant {
        identity: Option<u64>,
        name: String,
        payload: Vec<PackageReviewDataField>,
        retired_payload_identities: Vec<u64>,
    },
}

/// Closed semantic form of one public data declaration. Quotient identity is
/// the carrier family plus relation declaration; the proof implementation that
/// licensed formation is intentionally not API identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageReviewDataKind {
    Ordinary,
    Quotient {
        carrier: PackageReviewTypeIdentity,
        relation: PackageReviewNominalIdentity,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageReviewDataShape {
    pub(crate) identity: PackageReviewNominalIdentity,
    pub(crate) kind: PackageReviewDataKind,
    pub(crate) supply: psi_language_semantics::DataSupplyMode,
    pub(crate) lifetime_parameter_count: usize,
    pub(crate) type_parameters: Vec<PackageReviewTypeParameter>,
    pub(crate) properties: psi_typed_trees::data::DataProperties,
    pub(crate) zero_gated: bool,
    pub(crate) invariants: Vec<PackageReviewContractFact>,
    pub(crate) retired_identities: Vec<u64>,
    pub(crate) members: Vec<PackageReviewDataMember>,
}

/// The representation/ABI commitment retained for an opaque boundary datum.
/// Review projection currently has no sealed realization join, so it can only
/// state that the commitment is absent rather than inventing a layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewRepresentationAbiCommitment {
    Unbound,
}

/// The selected external representation mechanism for an opaque boundary
/// datum. Mechanism selection is not yet joined into checked package review.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewRepresentationMechanism {
    Unbound,
}

/// Distinct representation-TCB evidence for one package-owned opaque boundary
/// datum. This row is emitted independently of visibility, claims, and reach:
/// none of those facts can make an externally supplied representation cease to
/// be trusted implementation surface.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewRepresentationTcb {
    pub(crate) declaration: PackageReviewNominalIdentity,
    pub(crate) abi: PackageReviewRepresentationAbiCommitment,
    pub(crate) mechanism: PackageReviewRepresentationMechanism,
}

impl PackageReviewRepresentationTcb {
    pub const fn declaration(&self) -> &PackageReviewNominalIdentity {
        &self.declaration
    }

    pub const fn abi(&self) -> PackageReviewRepresentationAbiCommitment {
        self.abi
    }

    pub const fn mechanism(&self) -> PackageReviewRepresentationMechanism {
        self.mechanism
    }
}

impl PackageReviewDataShape {
    pub const fn identity(&self) -> &PackageReviewNominalIdentity {
        &self.identity
    }

    pub const fn kind(&self) -> &PackageReviewDataKind {
        &self.kind
    }

    pub const fn supply(&self) -> psi_language_semantics::DataSupplyMode {
        self.supply
    }

    pub const fn lifetime_parameter_count(&self) -> usize {
        self.lifetime_parameter_count
    }

    pub fn type_parameters(&self) -> &[PackageReviewTypeParameter] {
        &self.type_parameters
    }

    pub const fn properties(&self) -> psi_typed_trees::data::DataProperties {
        self.properties
    }

    pub const fn zero_gated(&self) -> bool {
        self.zero_gated
    }

    pub fn invariants(&self) -> &[PackageReviewContractFact] {
        &self.invariants
    }

    pub fn retired_identities(&self) -> &[u64] {
        &self.retired_identities
    }

    pub fn members(&self) -> &[PackageReviewDataMember] {
        &self.members
    }
}
