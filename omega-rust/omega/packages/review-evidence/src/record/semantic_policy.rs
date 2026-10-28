//! Source-semantic dependencies without private consumer implementation names.

use super::{
    PackageReviewNominalIdentity, PackageReviewSemanticDependencyExposure,
    PackageReviewSemanticDependencyKind,
};
use crate::record::accessors::field_projection_accessors;

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
    field_projection_accessors!(borrowed consumer: &PackagePolicySemanticDependencyConsumer);
    field_projection_accessors!(borrowed dependency: &PackageReviewNominalIdentity);
    field_projection_accessors!(copied exposure: PackageReviewSemanticDependencyExposure);
    field_projection_accessors!(copied kind: PackageReviewSemanticDependencyKind);
}
