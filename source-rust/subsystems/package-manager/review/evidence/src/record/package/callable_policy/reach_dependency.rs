// Normalized finite union of concrete services and static-telescope parameter
// ordinals (`PackagePolicyServiceReachDependency`).

use crate::record::PackageReviewNominalIdentity;
use crate::record::accessors::field_projection_accessors;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PackagePolicyServiceReachDependency {
    pub(crate) concrete: Vec<PackageReviewNominalIdentity>,
    pub(crate) parameters: Vec<u32>,
}

impl PackagePolicyServiceReachDependency {
    field_projection_accessors!(sliced concrete: &[PackageReviewNominalIdentity]);
    field_projection_accessors!(sliced parameters: &[u32]);
}
