// Bounded recovery for synchronous invocation summaries.

use super::{Error, reader::Reader};
use crate::encoding::recovery::policy::identity::service;
use crate::record::PackageReviewSynchronousInvocation;

pub(in crate::encoding::recovery) fn synchronous_invocation(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewSynchronousInvocation, Error> {
    Ok(super::tag!(reader => {
        0 => PackageReviewSynchronousInvocation::Parameter(reader.u32()?),
        1 => PackageReviewSynchronousInvocation::Service(service(reader)?),
    }))
}
