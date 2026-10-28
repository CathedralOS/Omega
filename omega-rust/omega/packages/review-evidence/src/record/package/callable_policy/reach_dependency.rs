use crate::record::PackageReviewNominalIdentity;
use crate::record::accessors::field_projection_accessors;

/// A normalized finite union, not another service-row expression language.
/// Each parameter ordinal selects an exact nominal contract in the callable's
/// complete static telescope, which already retains its trait and requirement.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PackagePolicyServiceReachDependency {
    pub(crate) concrete: Vec<PackageReviewNominalIdentity>,
    pub(crate) parameters: Vec<u32>,
}

impl PackagePolicyServiceReachDependency {
    field_projection_accessors!(sliced concrete: &[PackageReviewNominalIdentity]);
    field_projection_accessors!(sliced parameters: &[u32]);
}
