use crate::record::PackageReviewNominalIdentity;

/// A normalized finite union, not another service-row expression language.
/// Each parameter ordinal selects an exact nominal contract in the callable's
/// complete static telescope, which already retains its trait and requirement.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PackagePolicyServiceReachDependency {
    pub(crate) concrete: Vec<PackageReviewNominalIdentity>,
    pub(crate) parameters: Vec<u32>,
}

impl PackagePolicyServiceReachDependency {
    pub fn concrete(&self) -> &[PackageReviewNominalIdentity] {
        &self.concrete
    }

    pub fn parameters(&self) -> &[u32] {
        &self.parameters
    }
}
