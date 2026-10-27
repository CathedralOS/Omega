use super::super::encoder::Encoder;
use crate::encoding::PackageReviewEncodingError;
use crate::record::{PackageReviewInstallationReach, PackageReviewSynchronousInvocation};

use super::identity::encode_nominal;

pub(crate) fn encode_synchronous_invocation(
    encoder: &mut Encoder,
    invocation: &PackageReviewSynchronousInvocation,
) -> Result<(), PackageReviewEncodingError> {
    match invocation {
        PackageReviewSynchronousInvocation::Parameter(position) => {
            encoder.tag("parameter", 0);
            encoder.u32_field("position", *position)?;
        }
        PackageReviewSynchronousInvocation::Service(service) => {
            encoder.tag("service", 1);
            encoder.field("service", |encoder| encode_nominal(encoder, service))?;
        }
    }
    Ok(())
}
pub(crate) fn encode_installation_reach(
    encoder: &mut Encoder,
    reach: &PackageReviewInstallationReach,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field_by("requirement", &reach.requirement, encode_nominal)?;
    encoder.field("upper_bound", |encoder| {
        encoder.sequence(&reach.upper_bound, encode_nominal)
    })
}
