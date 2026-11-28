// Bounded recovery for synchronous invocation summaries.

use super::{Error, reader::Reader};
use crate::encoding::recovery::policy::identity::service;
use crate::record::PackageReviewSynchronousInvocation;

pub(in crate::encoding::recovery) fn projected_parameter<S>(
    reader: &mut Reader<'_>,
) -> Result<semantic_vocabulary::SynchronousInvocation<S>, Error> {
    use semantic_vocabulary::InvocationPathSegment as Step;
    let parameter = reader.u32()?;
    let path = reader.sequence(1, |reader| {
        Ok(super::tag!(reader => {
            1 => Step::Field(reader.string()?),
            2 => Step::FixedIndex(reader.u64()?),
            3 => Step::Case(reader.string()?),
            4 => Step::Referent,
        }))
    })?;
    Ok(semantic_vocabulary::SynchronousInvocation::ProjectedParameter { parameter, path })
}
pub(in crate::encoding::recovery) fn synchronous_invocation(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewSynchronousInvocation, Error> {
    Ok(super::tag!(reader => {
        0 => PackageReviewSynchronousInvocation::Parameter(reader.u32()?),
        1 => PackageReviewSynchronousInvocation::Service(service(reader)?),
        2 => projected_parameter(reader)?,
    }))
}

#[test]
fn review_recovery_keeps_the_exact_invoked_field_and_refuses_a_partial_path() {
    use crate::encoding::PackagePolicyRecoveryLimits;
    use crate::encoding::encode::{encode_synchronous_invocation, encoder::Encoder};
    use semantic_vocabulary::InvocationPathSegment as Step;

    let row = PackageReviewSynchronousInvocation::ProjectedParameter {
        parameter: 1,
        path: vec![
            Step::Referent,
            Step::Field("operations".into()),
            Step::FixedIndex(3),
            Step::Case("Available".into()),
            Step::Field("read".into()),
        ],
    };
    let mut writer = Encoder::bounded(256);
    encode_synchronous_invocation(&mut writer, &row).unwrap();
    let bytes = writer.finish().unwrap();
    let mut reader = Reader::new(&bytes, PackagePolicyRecoveryLimits::default()).unwrap();
    assert_eq!(synchronous_invocation(&mut reader).unwrap(), row);
    reader.finish().unwrap();
    let mut truncated = Reader::new(
        &bytes[..bytes.len() - 1],
        PackagePolicyRecoveryLimits::default(),
    )
    .unwrap();
    assert!(synchronous_invocation(&mut truncated).is_err());
}
