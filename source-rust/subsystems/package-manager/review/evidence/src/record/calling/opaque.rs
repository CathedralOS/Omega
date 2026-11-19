// Selected opaque meanings actually used by one exact calling signature.
//
// The containing calling policy owns the complete shape graph, and occurrence
// roots refer to that graph rather than to compiler handles or equal-looking
// shapes. Unused selections and producer availability are not calling-use rows.

use crate::record::{
    PackagePolicyClosedConformanceApplication, PackageReviewNominalIdentity,
    PackageReviewNominalOwner, PackageReviewOpaqueRepresentationApplicationOrigin,
    PackageReviewOpaqueRepresentationCopyDisposition,
    PackageReviewOpaqueRepresentationLifecycleDisposition,
    PackageReviewOpaqueRepresentationOccurrence,
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct PackagePolicyCallingOpaqueUse {
    pub(crate) opaque: PackageReviewNominalIdentity,
    pub(crate) carrier: PackageReviewNominalIdentity,
    pub(crate) selection_owner: PackageReviewNominalOwner,
    pub(crate) application: PackagePolicyClosedConformanceApplication,
    pub(crate) origin: PackageReviewOpaqueRepresentationApplicationOrigin,
    pub(crate) lifecycle: PackageReviewOpaqueRepresentationLifecycleDisposition,
    pub(crate) copy_disposition: PackageReviewOpaqueRepresentationCopyDisposition,
    pub(crate) occurrences: Vec<PackageReviewOpaqueRepresentationOccurrence>,
}
