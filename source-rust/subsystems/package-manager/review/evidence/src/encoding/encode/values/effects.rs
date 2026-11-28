// Canonical encoding for synchronous callable invocations and installation reach bounds.

use super::super::encoder::Encoder;
use crate::encoding::PackageReviewEncodingError;
use crate::encoding::encode::values::identity::encode_service;
use crate::record::{PackageReviewInstallationReach, PackageReviewSynchronousInvocation};

use super::identity::encode_nominal;

pub(in crate::encoding::encode) fn encode_invocation_path(
    encoder: &mut Encoder,
    parameter: u32,
    path: &[semantic_vocabulary::InvocationPathSegment],
) -> Result<(), PackageReviewEncodingError> {
    use semantic_vocabulary::InvocationPathSegment as Step;
    encoder.u32_field("position", parameter)?;
    encoder.sequence_field("path", path, |encoder, step| match step {
        Step::Field(name) => {
            encoder.tag("field", 1);
            encoder.string_field("identity", name)
        }
        Step::FixedIndex(index) => {
            encoder.tag("fixed_index", 2);
            encoder.u64_field("index", *index)
        }
        Step::Case(name) => {
            encoder.tag("case", 3);
            encoder.string_field("identity", name)
        }
        Step::Referent => {
            encoder.tag("referent", 4);
            Ok(())
        }
    })
}
pub(crate) fn encode_synchronous_invocation(
    encoder: &mut Encoder,
    invocation: &PackageReviewSynchronousInvocation,
) -> Result<(), PackageReviewEncodingError> {
    match invocation {
        PackageReviewSynchronousInvocation::Parameter(position) => {
            encoder.tag("parameter", 0);
            encoder.u32_field("position", *position)?;
        }
        PackageReviewSynchronousInvocation::ProjectedParameter { parameter, path } => {
            encoder.tag("projected_parameter", 2);
            encode_invocation_path(encoder, *parameter, path)?;
        }
        PackageReviewSynchronousInvocation::Service(service) => {
            encoder.tag("service", 1);
            encoder.field("service", |encoder| encode_service(encoder, service))?;
        }
    }
    Ok(())
}
pub(crate) fn encode_installation_reach(
    encoder: &mut Encoder,
    reach: &PackageReviewInstallationReach,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field_by("requirement", &reach.requirement, encode_nominal)?;
    encoder.sequence_field("upper_bound", &reach.upper_bound, encode_service)
}
