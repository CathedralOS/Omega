use super::{Error, identity::nominal, reader::Reader};
use crate::record::PackageReviewSynchronousInvocation;

pub(in crate::encoding::recovery) fn synchronous_invocation(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewSynchronousInvocation, Error> {
    Ok(match reader.byte()? {
        0 => PackageReviewSynchronousInvocation::Parameter(reader.u32()?),
        1 => PackageReviewSynchronousInvocation::Service(nominal(reader)?),
        _ => return Err(Error::InvalidTag),
    })
}
