use super::super::PackageReviewEncodingError;
use super::super::encoder::Encoder;
use super::super::values::identity::encode_nominal;
use super::data::encode_type_identity;
use crate::record::PackageReviewConformanceBound;

pub(crate) fn encode_conformance_bound(
    encoder: &mut Encoder,
    bound: &PackageReviewConformanceBound,
) -> Result<(), PackageReviewEncodingError> {
    encoder.u32_field("binder_ordinal", bound.binder_ordinal)?;
    encoder.field_by("trait_identity", &bound.trait_identity, encode_nominal)?;
    encoder.field("trait_lifetime_arguments", |encoder| {
        encoder.sequence(&bound.trait_lifetime_arguments, |encoder, argument| {
            encoder.u32_field("argument", *argument)?;
            Ok(())
        })
    })?;
    encoder.field("arguments", |encoder| {
        encoder.sequence(&bound.arguments, encode_type_identity)
    })
}
