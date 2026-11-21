// Normalized finite union of concrete services and static-telescope parameter
// ordinals (`PackagePolicyServiceReachDependency`).

use crate::record::PackageReviewServiceIdentity;
use crate::record::accessors::field_projection_accessors;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PackagePolicyServiceReachDependency {
    pub concrete: Vec<PackageReviewServiceIdentity>,
    pub parameters: Vec<u32>,
}

impl PackagePolicyServiceReachDependency {
    field_projection_accessors!(sliced concrete: &[PackageReviewServiceIdentity]);
    field_projection_accessors!(sliced parameters: &[u32]);
}
