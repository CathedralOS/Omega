use crate::record::PackageReviewNominalIdentity;
use crate::record::PackageReviewTypeIdentity;
use crate::record::accessors::field_projection_accessors;

/// Exact caller lifetime choices remain distinct from the requirement's
/// alpha-normalized equality partition.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackagePolicyCallableConformance {
    pub(crate) trait_identity: PackageReviewNominalIdentity,
    pub(crate) requirement_identity: PackageReviewNominalIdentity,
    pub(crate) requirement_lifetime_partition: Vec<u32>,
    pub(crate) trait_lifetime_arguments: Vec<u32>,
    pub(crate) arguments: Vec<PackageReviewTypeIdentity>,
    pub(crate) alias: Option<String>,
}

impl PackagePolicyCallableConformance {
    field_projection_accessors!(borrowed trait_identity: &PackageReviewNominalIdentity);
    field_projection_accessors!(borrowed requirement_identity: &PackageReviewNominalIdentity);
    field_projection_accessors!(sliced requirement_lifetime_partition: &[u32]);
    field_projection_accessors!(sliced trait_lifetime_arguments: &[u32]);
    field_projection_accessors!(sliced arguments: &[PackageReviewTypeIdentity]);
    pub fn alias(&self) -> Option<&str> {
        self.alias.as_deref()
    }
}
