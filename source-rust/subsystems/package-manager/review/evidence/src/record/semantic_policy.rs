//! A declaration selected by source and a nominal carried through another API
//! have different dependency contracts. A carried type commits to its supplier's
//! whole checked source occurrence without granting source selection.
use super::{
    PackageReviewNominalIdentity, PackageReviewSemanticDependencyExposure,
    PackageReviewSemanticDependencyKind,
};
use source_provenance::PackageSourceConsumptionCommitment;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackagePolicySemanticDependencyConsumer {
    Callable(PackageReviewNominalIdentity),
    PackageImplementation,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackagePolicySemanticDependency {
    Declaration {
        consumer: PackagePolicySemanticDependencyConsumer,
        dependency: PackageReviewNominalIdentity,
        exposure: PackageReviewSemanticDependencyExposure,
        kind: PackageReviewSemanticDependencyKind,
    },
    CarriedType {
        dependency: PackageReviewNominalIdentity,
        supplier_sources: PackageSourceConsumptionCommitment,
        exposure: PackageReviewSemanticDependencyExposure,
    },
}

impl PackagePolicySemanticDependency {
    pub fn dependency(&self) -> &PackageReviewNominalIdentity {
        match self {
            Self::Declaration { dependency, .. } | Self::CarriedType { dependency, .. } => {
                dependency
            }
        }
    }

    pub fn exposure(&self) -> PackageReviewSemanticDependencyExposure {
        match self {
            Self::Declaration { exposure, .. } | Self::CarriedType { exposure, .. } => *exposure,
        }
    }
}
