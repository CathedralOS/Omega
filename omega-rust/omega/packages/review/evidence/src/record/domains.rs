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
    ExactMachine,
}

/// Inert issuer identity. A concrete machine has no invented trait or
/// requirement; no variant grants invocation or package admission.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewDomainEstablishmentRoute {
    CheckedRequirement {
        trait_identity: PackageReviewNominalIdentity,
        requirement_identity: PackageReviewNominalIdentity,
    },
    BoundaryRequirement {
        trait_identity: PackageReviewNominalIdentity,
        requirement_identity: PackageReviewNominalIdentity,
    },
    ExactMachine {
        machine_identity: PackageReviewNominalIdentity,
    },
}

impl PackageReviewDomainEstablishmentRoute {
    pub const fn kind(&self) -> PackageReviewDomainEstablishmentKind {
        match self {
            Self::CheckedRequirement { .. } => {
                PackageReviewDomainEstablishmentKind::CheckedRequirement
            }
            Self::BoundaryRequirement { .. } => {
                PackageReviewDomainEstablishmentKind::BoundaryRequirement
            }
            Self::ExactMachine { .. } => PackageReviewDomainEstablishmentKind::ExactMachine,
        }
    }

    pub const fn trait_identity(&self) -> Option<&PackageReviewNominalIdentity> {
        match self {
            Self::CheckedRequirement { trait_identity, .. }
            | Self::BoundaryRequirement { trait_identity, .. } => Some(trait_identity),
            Self::ExactMachine { .. } => None,
        }
    }

    pub const fn requirement_identity(&self) -> Option<&PackageReviewNominalIdentity> {
        match self {
            Self::CheckedRequirement {
                requirement_identity,
                ..
            }
            | Self::BoundaryRequirement {
                requirement_identity,
                ..
            } => Some(requirement_identity),
            Self::ExactMachine { .. } => None,
        }
    }

    pub const fn machine_identity(&self) -> Option<&PackageReviewNominalIdentity> {
        match self {
            Self::ExactMachine { machine_identity } => Some(machine_identity),
            Self::CheckedRequirement { .. } | Self::BoundaryRequirement { .. } => None,
        }
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
