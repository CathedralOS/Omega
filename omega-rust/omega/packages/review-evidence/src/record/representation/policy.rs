// Complete inert representation policy, availability, selections, and calling demands for one reviewed package and target.

mod validation;

use crate::record::accessors::field_projection_accessors;
use crate::record::{
    PackagePolicyCallingPlan, PackagePolicyClosedConformanceApplication,
    PackagePolicyConformanceShape, PackageReviewNominalIdentity, PackageReviewNominalOwner,
    PackageReviewOpaqueRepresentationApplicationOrigin,
    PackageReviewOpaqueRepresentationCopyDisposition,
    PackageReviewOpaqueRepresentationLifecycleDisposition, PackageReviewRepresentationTarget,
};
use semantic_vocabulary::PackageKeyIdentity;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicyRepresentation {
    pub(crate) package: PackageKeyIdentity,
    pub(crate) target: PackageReviewRepresentationTarget,
    pub(crate) declarations: Vec<PackageReviewNominalIdentity>,
    pub(crate) producer_availability: Vec<PackagePolicyRepresentationAvailability>,
    pub(crate) selected_availability: Vec<PackagePolicyRepresentationSelection>,
    pub(crate) demands: Vec<PackagePolicyRepresentationDemand>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicyRepresentationAvailability {
    pub(crate) opaque: PackageReviewNominalIdentity,
    pub(crate) conformance: PackagePolicyConformanceShape,
    pub(crate) carrier: PackageReviewNominalIdentity,
}

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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackagePolicyRepresentationDemand {
    pub(crate) opaque: PackageReviewNominalIdentity,
    pub(crate) calling: PackagePolicyCallingPlan,
}

impl PackagePolicyRepresentation {
    field_projection_accessors!(copied package: semantic_vocabulary::PackageKeyIdentity);
    field_projection_accessors!(copied target: PackageReviewRepresentationTarget);
    field_projection_accessors!(sliced declarations: &[PackageReviewNominalIdentity]);
    field_projection_accessors!(sliced producer_availability: &[PackagePolicyRepresentationAvailability]);
    field_projection_accessors!(sliced selected_availability: &[PackagePolicyRepresentationSelection]);
    field_projection_accessors!(sliced demands: &[PackagePolicyRepresentationDemand]);
}

impl PackagePolicyRepresentationAvailability {
    field_projection_accessors!(borrowed opaque: &PackageReviewNominalIdentity);
    field_projection_accessors!(borrowed conformance: &PackagePolicyConformanceShape);
    field_projection_accessors!(borrowed carrier: &PackageReviewNominalIdentity);
}

impl PackagePolicyRepresentationSelection {
    crate::record::accessors::opaque_representation_selection_accessors!();
}

impl PackagePolicyRepresentationDemand {
    pub(crate) fn compare_application(&self, other: &Self) -> std::cmp::Ordering {
        self.compare_calling_application(other)
            .then_with(|| self.opaque.cmp(&other.opaque))
    }

    fn compare_calling_application(&self, other: &Self) -> std::cmp::Ordering {
        (
            &self.calling.abi,
            &self.calling.boundary_trait,
            &self.calling.boundary_arguments,
            self.calling.boundary_lifetime_parameter_count,
            &self.calling.boundary_lifetime_arguments,
            self.calling.application_lifetime_parameter_count,
            &self.calling.requirement,
        )
            .cmp(&(
                &other.calling.abi,
                &other.calling.boundary_trait,
                &other.calling.boundary_arguments,
                other.calling.boundary_lifetime_parameter_count,
                &other.calling.boundary_lifetime_arguments,
                other.calling.application_lifetime_parameter_count,
                &other.calling.requirement,
            ))
    }

    field_projection_accessors!(borrowed opaque: &PackageReviewNominalIdentity);
    field_projection_accessors!(borrowed calling: &PackagePolicyCallingPlan);
}
