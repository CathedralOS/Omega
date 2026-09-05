//! Stable public-domain evidence.

use super::{
    contracts::PackageReviewContractFact,
    identity::PackageReviewNominalIdentity,
    signatures::{PackageReviewTypeIdentity, PackageReviewTypeParameter},
};

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
    Carry(language_semantics::CarryPermission),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageReviewDomainShape {
    pub(crate) identity: PackageReviewNominalIdentity,
    pub(crate) type_parameters: Vec<PackageReviewTypeParameter>,
    pub(crate) target_type: PackageReviewTypeIdentity,
    pub(crate) index_arguments: Vec<PackageReviewTypeIdentity>,
    pub(crate) predicate_body: language_semantics::DomainPredicateBody,
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

    pub const fn predicate_body(&self) -> language_semantics::DomainPredicateBody {
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
