//! Source-semantic dependencies without private consumer implementation names.

use super::{
    PackageReviewNominalIdentity, PackageReviewSemanticDependencyExposure,
    PackageReviewSemanticDependencyKind,
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackagePolicySemanticDependencyConsumer {
    /// An exact callable retained by the enclosing normalized callable surface.
    Callable(PackageReviewNominalIdentity),
    /// Private consumers are grouped under the enclosing baseline's package.
    PackageImplementation,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackagePolicySemanticDependency {
    pub(crate) consumer: PackagePolicySemanticDependencyConsumer,
    pub(crate) dependency: PackageReviewNominalIdentity,
    pub(crate) exposure: PackageReviewSemanticDependencyExposure,
    pub(crate) kind: PackageReviewSemanticDependencyKind,
}

impl PackagePolicySemanticDependency {
    pub const fn consumer(&self) -> &PackagePolicySemanticDependencyConsumer {
        &self.consumer
    }
    pub const fn dependency(&self) -> &PackageReviewNominalIdentity {
        &self.dependency
    }
    pub const fn exposure(&self) -> PackageReviewSemanticDependencyExposure {
        self.exposure
    }
    pub const fn kind(&self) -> PackageReviewSemanticDependencyKind {
        self.kind
    }
}
