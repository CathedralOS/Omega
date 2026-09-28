use super::{Error, Reader, nominal};
use crate::encoding::recovery::policy::{
    conformance, identity::owner, physical_calling_policy::placement::value_placement,
};
use crate::record::{
    PackagePolicyCallingOpaqueUse, PackageReviewOpaqueRepresentationApplicationOrigin,
    PackageReviewOpaqueRepresentationCopyDisposition,
    PackageReviewOpaqueRepresentationLifecycleDisposition,
    PackageReviewOpaqueRepresentationMovementRole, PackageReviewOpaqueRepresentationOccurrence,
    PackageReviewOpaqueRepresentationPathElement,
};

pub(super) fn decode(reader: &mut Reader<'_>) -> Result<PackagePolicyCallingOpaqueUse, Error> {
    Ok(PackagePolicyCallingOpaqueUse {
        opaque: nominal(reader)?,
        carrier: nominal(reader)?,
        selection_owner: owner(reader)?,
        application: conformance::application(reader)?,
        origin: match reader.byte()? {
            0 => PackageReviewOpaqueRepresentationApplicationOrigin::NamedConformance,
            _ => return Err(Error::InvalidTag),
        },
        lifecycle: match reader.byte()? {
            0 => PackageReviewOpaqueRepresentationLifecycleDisposition::Inert,
            _ => return Err(Error::InvalidTag),
        },
        copy_disposition: match reader.byte()? {
            0 => PackageReviewOpaqueRepresentationCopyDisposition::PlacementOnly,
            1 => PackageReviewOpaqueRepresentationCopyDisposition::CheckedSemanticCopy,
            _ => return Err(Error::InvalidTag),
        },
        occurrences: reader.sequence(24, |reader| {
            Ok(PackageReviewOpaqueRepresentationOccurrence {
                carrier_shape_root: reader.u16()?,
                role: match reader.byte()? {
                    0 => PackageReviewOpaqueRepresentationMovementRole::Parameter {
                        formal_ordinal: reader.u32()?,
                        native_ordinal: reader.u32()?,
                    },
                    1 => PackageReviewOpaqueRepresentationMovementRole::Result,
                    _ => return Err(Error::InvalidTag),
                },
                path: reader.sequence(1, |reader| {
                    Ok(match reader.byte()? {
                        0 => PackageReviewOpaqueRepresentationPathElement::FixedArrayElement,
                        1 => PackageReviewOpaqueRepresentationPathElement::RecordField {
                            ordinal: reader.u16()?,
                        },
                        _ => return Err(Error::InvalidTag),
                    })
                })?,
                placement: value_placement(reader)?,
            })
        })?,
    })
}
