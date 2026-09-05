//! Complete inert representation policy for one reviewed package and target.

mod validation;

use crate::record::{
    PackagePolicyCallingPlan, PackagePolicyClosedConformanceApplication,
    PackageReviewConformanceShape, PackageReviewNominalIdentity, PackageReviewNominalOwner,
    PackageReviewOpaqueRepresentationApplicationOrigin,
    PackageReviewOpaqueRepresentationCopyDisposition,
    PackageReviewOpaqueRepresentationLifecycleDisposition, PackageReviewRepresentationTarget,
};
use psi_core::PackageKeyIdentity;

/// Declarations and producer candidates do not accept a selection. Selected
/// availability includes unused choices; only actual crossings create demands.
/// Every collection is canonical and independently owned by the reviewed package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicyRepresentation {
    pub(crate) package: PackageKeyIdentity,
    pub(crate) target: PackageReviewRepresentationTarget,
    pub(crate) declarations: Vec<PackageReviewNominalIdentity>,
    pub(crate) producer_availability: Vec<PackagePolicyRepresentationAvailability>,
    pub(crate) selected_availability: Vec<PackagePolicyRepresentationSelection>,
    pub(crate) demands: Vec<PackagePolicyRepresentationDemand>,
}

/// An exact public candidate, including its uninstantiated declaration telescope.
/// This contains no invented closed application or consumer selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicyRepresentationAvailability {
    pub(crate) opaque: PackageReviewNominalIdentity,
    pub(crate) conformance: PackageReviewConformanceShape,
    pub(crate) carrier: PackageReviewNominalIdentity,
}

/// One independently rederived activation-wide selection. The authoritative
/// build machine has no lifetime telescope, so its application is lifetime-free.
/// Its package owner is policy; the build machine's name and span are not.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackagePolicyRepresentationSelection {
    pub(crate) opaque: PackageReviewNominalIdentity,
    pub(crate) carrier: PackageReviewNominalIdentity,
    pub(crate) selection_owner: PackageReviewNominalOwner,
    pub(crate) application: PackagePolicyClosedConformanceApplication,
    pub(crate) origin: PackageReviewOpaqueRepresentationApplicationOrigin,
    pub(crate) lifecycle: PackageReviewOpaqueRepresentationLifecycleDisposition,
    pub(crate) copy_disposition: PackageReviewOpaqueRepresentationCopyDisposition,
}

/// One opaque declaration's actual use in a complete checked calling application.
/// The calling graph owns every occurrence, path, and physical placement.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackagePolicyRepresentationDemand {
    pub(crate) opaque: PackageReviewNominalIdentity,
    pub(crate) calling: PackagePolicyCallingPlan,
}

impl PackagePolicyRepresentation {
    pub const fn package(&self) -> PackageKeyIdentity {
        self.package
    }

    pub const fn target(&self) -> PackageReviewRepresentationTarget {
        self.target
    }

    pub fn declarations(&self) -> &[PackageReviewNominalIdentity] {
        &self.declarations
    }

    pub fn producer_availability(&self) -> &[PackagePolicyRepresentationAvailability] {
        &self.producer_availability
    }

    pub fn selected_availability(&self) -> &[PackagePolicyRepresentationSelection] {
        &self.selected_availability
    }

    pub fn demands(&self) -> &[PackagePolicyRepresentationDemand] {
        &self.demands
    }
}

impl PackagePolicyRepresentationAvailability {
    pub const fn opaque(&self) -> &PackageReviewNominalIdentity {
        &self.opaque
    }

    pub const fn conformance(&self) -> &PackageReviewConformanceShape {
        &self.conformance
    }

    pub const fn carrier(&self) -> &PackageReviewNominalIdentity {
        &self.carrier
    }
}

impl PackagePolicyRepresentationSelection {
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
}

impl PackagePolicyRepresentationDemand {
    pub(crate) fn compare_application(&self, other: &Self) -> std::cmp::Ordering {
        (
            &self.calling.boundary_trait,
            &self.calling.boundary_arguments,
            &self.calling.requirement,
            &self.opaque,
        )
            .cmp(&(
                &other.calling.boundary_trait,
                &other.calling.boundary_arguments,
                &other.calling.requirement,
                &other.opaque,
            ))
    }

    pub const fn opaque(&self) -> &PackageReviewNominalIdentity {
        &self.opaque
    }

    pub const fn calling(&self) -> &PackagePolicyCallingPlan {
        &self.calling
    }
}
