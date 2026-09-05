use super::*;

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
    pub fn trait_identity(&self) -> &PackageReviewNominalIdentity {
        &self.trait_identity
    }
    pub fn requirement_identity(&self) -> &PackageReviewNominalIdentity {
        &self.requirement_identity
    }
    pub fn requirement_lifetime_partition(&self) -> &[u32] {
        &self.requirement_lifetime_partition
    }
    pub fn trait_lifetime_arguments(&self) -> &[u32] {
        &self.trait_lifetime_arguments
    }
    pub fn arguments(&self) -> &[PackageReviewTypeIdentity] {
        &self.arguments
    }
    pub fn alias(&self) -> Option<&str> {
        self.alias.as_deref()
    }
}
