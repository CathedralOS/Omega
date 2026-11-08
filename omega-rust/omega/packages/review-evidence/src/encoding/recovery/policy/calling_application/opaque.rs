use super::{Error, Reader};
use crate::encoding::recovery::policy::physical_calling_policy::placement::value_placement;
use crate::record::{
    PackagePolicyCallingOpaqueUse, PackageReviewOpaqueRepresentationMovementRole,
    PackageReviewOpaqueRepresentationOccurrence, PackageReviewOpaqueRepresentationPathElement,
};

pub(in crate::encoding::recovery) fn decode(
    reader: &mut Reader<'_>,
) -> Result<PackagePolicyCallingOpaqueUse, Error> {
    let (opaque, carrier, selection_owner, application, origin, lifecycle, copy_disposition) =
        super::super::representation::opaque_representation_fields(reader)?;
    Ok(PackagePolicyCallingOpaqueUse {
        opaque,
        carrier,
        selection_owner,
        application,
        origin,
        lifecycle,
        copy_disposition,
        occurrences: reader.sequence(24, |reader| {
            Ok(PackageReviewOpaqueRepresentationOccurrence {
                carrier_shape_root: reader.u16()?,
                role: super::super::tag!(reader => {
                    0 => PackageReviewOpaqueRepresentationMovementRole::Parameter {
                        formal_ordinal: reader.u32()?,
                        native_ordinal: reader.u32()?,
                    },
                    1 => PackageReviewOpaqueRepresentationMovementRole::Result,
                }),
                path: reader.sequence(1, |reader| {
                    Ok(super::super::tag!(reader => {
                        0 => PackageReviewOpaqueRepresentationPathElement::FixedArrayElement,
                        1 => PackageReviewOpaqueRepresentationPathElement::RecordField {
                            ordinal: reader.u16()?,
                        },
                    }))
                })?,
                placement: value_placement(reader)?,
            })
        })?,
    })
}
