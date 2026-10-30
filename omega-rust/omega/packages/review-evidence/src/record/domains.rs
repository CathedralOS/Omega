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
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum PackageReviewDomainAliasAtom {
    Declared(PackageReviewNominalIdentity),
    Carry(language_semantics::CarryPermission),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PackageReviewDomainShape {
    pub(crate) identity: PackageReviewNominalIdentity,
    pub(crate) type_parameters: Vec<PackageReviewTypeParameter>,
    pub(crate) target_type: PackageReviewTypeIdentity,
    pub(crate) index_arguments: Vec<PackageReviewTypeIdentity>,
    pub(crate) predicate_presence: language_semantics::DomainPredicatePresence,
    pub(crate) predicate_facts: Vec<PackageReviewContractFact>,
    pub(crate) alias_expansion: Option<Vec<PackageReviewDomainAliasAtom>>,
    pub(crate) classification: Option<PackageReviewDomainClassification>,
    pub(crate) semantic_roles: Vec<PackageReviewDomainSemanticRole>,
    pub(crate) establishment_routes: Vec<PackageReviewDomainEstablishmentRoute>,
}
