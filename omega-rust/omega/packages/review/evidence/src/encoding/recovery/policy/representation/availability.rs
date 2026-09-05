use super::{Error, Reader, nominal};
use crate::encoding::recovery::policy::{
    contracts::evidence_interface, signatures::type_parameter,
};
use crate::record::{
    PackagePolicyRepresentationAvailability, PackageReviewConformanceShape,
    PackageReviewConformanceSubject,
};

pub(super) fn availability(
    reader: &mut Reader<'_>,
) -> Result<PackagePolicyRepresentationAvailability, Error> {
    Ok(PackagePolicyRepresentationAvailability {
        opaque: nominal(reader)?,
        conformance: conformance_shape(reader)?,
        carrier: nominal(reader)?,
    })
}

pub(super) fn conformance_shape(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewConformanceShape, Error> {
    Ok(PackageReviewConformanceShape {
        identity: nominal(reader)?,
        lifetime_parameter_count: reader.usize()?,
        type_parameters: reader.sequence(3, type_parameter)?,
        subject: match reader.byte()? {
            0 => PackageReviewConformanceSubject::Subjectless,
            1 => PackageReviewConformanceSubject::TypeParameter(reader.u32()?),
            2 => PackageReviewConformanceSubject::Nominal(nominal(reader)?),
            _ => return Err(Error::InvalidTag),
        },
        interface: evidence_interface(reader)?,
    })
}
