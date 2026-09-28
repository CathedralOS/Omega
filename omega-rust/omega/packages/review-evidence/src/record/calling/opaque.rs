//! Selected opaque meanings actually used by one exact calling signature.

use crate::record::{
    PackagePolicyClosedConformanceApplication, PackageReviewNominalIdentity,
    PackageReviewNominalOwner, PackageReviewOpaqueRepresentationApplicationOrigin,
    PackageReviewOpaqueRepresentationCopyDisposition,
    PackageReviewOpaqueRepresentationLifecycleDisposition,
    PackageReviewOpaqueRepresentationOccurrence,
};

/// The containing calling policy owns the complete shape graph. Occurrence
/// roots refer to that graph, not to compiler handles or equal-looking shapes.
/// Unused selections and producer availability are not calling-use rows.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackagePolicyCallingOpaqueUse {
    pub(crate) opaque: PackageReviewNominalIdentity,
    pub(crate) carrier: PackageReviewNominalIdentity,
    pub(crate) selection_owner: PackageReviewNominalOwner,
    pub(crate) application: PackagePolicyClosedConformanceApplication,
    pub(crate) origin: PackageReviewOpaqueRepresentationApplicationOrigin,
    pub(crate) lifecycle: PackageReviewOpaqueRepresentationLifecycleDisposition,
    pub(crate) copy_disposition: PackageReviewOpaqueRepresentationCopyDisposition,
    pub(crate) occurrences: Vec<PackageReviewOpaqueRepresentationOccurrence>,
}

impl PackagePolicyCallingOpaqueUse {
    pub const fn opaque(&self) -> &PackageReviewNominalIdentity {
        &self.opaque
    }

    pub const fn carrier(&self) -> &PackageReviewNominalIdentity {
        &self.carrier
    }

    pub const fn selection_owner(&self) -> PackageReviewNominalOwner {
        self.selection_owner
    }

    pub const fn application(&self) -> &PackagePolicyClosedConformanceApplication {
        &self.application
    }

    pub const fn origin(&self) -> PackageReviewOpaqueRepresentationApplicationOrigin {
        self.origin
    }

    pub const fn lifecycle(&self) -> PackageReviewOpaqueRepresentationLifecycleDisposition {
        self.lifecycle
    }

    pub const fn copy_disposition(&self) -> PackageReviewOpaqueRepresentationCopyDisposition {
        self.copy_disposition
    }

    pub fn occurrences(&self) -> &[PackageReviewOpaqueRepresentationOccurrence] {
        &self.occurrences
    }
}
