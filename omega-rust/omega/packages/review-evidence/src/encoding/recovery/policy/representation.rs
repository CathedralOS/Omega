//! Bounded, source-independent recovery of complete representation policy.

pub(in crate::encoding::recovery) mod availability;

use super::{
    Error, PackagePolicyRecoveryLimits, calling_application, conformance,
    identity::{nominal, owner, package},
    reader::Reader,
};
use crate::encoding::{PACKAGE_REPRESENTATION_POLICY_VERSION, REPRESENTATION_POLICY_MAGIC};
use crate::record::{
    PackagePolicyRepresentation, PackagePolicyRepresentationDemand,
    PackagePolicyRepresentationSelection, PackageReviewOpaqueRepresentationApplicationOrigin,
    PackageReviewOpaqueRepresentationCopyDisposition,
    PackageReviewOpaqueRepresentationLifecycleDisposition,
};

impl PackagePolicyRepresentation {
    super::recover_canonical_component!(
        REPRESENTATION_POLICY_MAGIC,
        PACKAGE_REPRESENTATION_POLICY_VERSION,
        policy,
        validate,
    );
}

pub(in crate::encoding::recovery) fn policy(
    reader: &mut Reader<'_>,
) -> Result<PackagePolicyRepresentation, Error> {
    Ok(PackagePolicyRepresentation {
        package: package(reader)?,
        target: calling_application::shapes::target(reader)?,
        declarations: reader.sequence(41, nominal)?,
        producer_availability: reader.sequence(1, availability::availability)?,
        selected_availability: reader.sequence(1, selection)?,
        demands: reader.sequence(1, |reader| {
            Ok(PackagePolicyRepresentationDemand {
                opaque: nominal(reader)?,
                calling: calling_application::application(reader)?,
            })
        })?,
    })
}

pub(in crate::encoding::recovery) fn opaque_representation_fields(
    reader: &mut Reader<'_>,
) -> Result<
    (
        crate::record::PackageReviewNominalIdentity,
        crate::record::PackageReviewNominalIdentity,
        crate::record::PackageReviewNominalOwner,
        crate::record::PackagePolicyClosedConformanceApplication,
        PackageReviewOpaqueRepresentationApplicationOrigin,
        PackageReviewOpaqueRepresentationLifecycleDisposition,
        PackageReviewOpaqueRepresentationCopyDisposition,
    ),
    Error,
> {
    Ok((
        nominal(reader)?,
        nominal(reader)?,
        owner(reader)?,
        conformance::application(reader)?,
        super::tag!(reader => {
            0 => PackageReviewOpaqueRepresentationApplicationOrigin::NamedConformance,
        }),
        super::tag!(reader => {
            0 => PackageReviewOpaqueRepresentationLifecycleDisposition::Inert,
        }),
        super::tag!(reader => {
            0 => PackageReviewOpaqueRepresentationCopyDisposition::PlacementOnly,
            1 => PackageReviewOpaqueRepresentationCopyDisposition::CheckedSemanticCopy,
        }),
    ))
}

pub(in crate::encoding::recovery) fn selection(
    reader: &mut Reader<'_>,
) -> Result<PackagePolicyRepresentationSelection, Error> {
    let (opaque, carrier, selection_owner, application, origin, lifecycle, copy_disposition) =
        opaque_representation_fields(reader)?;
    Ok(PackagePolicyRepresentationSelection {
        opaque,
        carrier,
        selection_owner,
        application,
        origin,
        lifecycle,
        copy_disposition,
    })
}
