// Stable public-domain evidence.
//
// `PackageReviewDomainSemanticRole` records closed compiler semantic roles
// without exporting compiler-private semantic-domain IDs across the canonical
// package-review boundary. `PackageReviewDomainEstablishmentRoute` records
// inert issuer identity without inventing traits for concrete machines or
// granting invocation/admission.

use crate::record::PackageReviewNominalIdentity;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageReviewDomainClassification {
    ProgressProfile,
}

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
