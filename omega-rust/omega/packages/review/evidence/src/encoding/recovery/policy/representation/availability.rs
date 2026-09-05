use super::{Error, Reader, nominal};
use crate::encoding::recovery::policy::public_api::conformance_shape;
use crate::record::PackagePolicyRepresentationAvailability;

pub(super) fn availability(
    reader: &mut Reader<'_>,
) -> Result<PackagePolicyRepresentationAvailability, Error> {
    Ok(PackagePolicyRepresentationAvailability {
        opaque: nominal(reader)?,
        conformance: conformance_shape(reader)?,
        carrier: nominal(reader)?,
    })
}
