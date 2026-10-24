use super::{Encoder, PackageReviewEncodingError};
use crate::encoding::encode::declarations::encode_opaque_occurrence;
use crate::encoding::encode::representation::encode_opaque_representation_fields;
use crate::encoding::encode::values::conformance_policy;
use crate::encoding::encode::values::identity::{encode_nominal, encode_selection_owner};
use crate::record::{
    PackagePolicyCallingOpaqueUse, PackageReviewOpaqueRepresentationApplicationOrigin,
    PackageReviewOpaqueRepresentationCopyDisposition,
    PackageReviewOpaqueRepresentationLifecycleDisposition,
};

pub(super) fn encode(
    encoder: &mut Encoder,
    use_: &PackagePolicyCallingOpaqueUse,
) -> Result<(), PackageReviewEncodingError> {
    encode_opaque_representation_fields!(
        encoder,
        use_,
        "calling policy has unresolved selection ownership"
    )?;
    encoder.field("occurrences", |encoder| {
        encoder.sequence(&use_.occurrences, encode_opaque_occurrence)
    })
}
