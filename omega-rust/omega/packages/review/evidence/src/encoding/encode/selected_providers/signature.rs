use super::{Encoder, PackageReviewEncodingError};
use crate::encoding::encode::declarations::encode_type_identity;
use crate::encoding::encode::public_api::type_parameter as encode_type_parameter;
use crate::record::PackagePolicyServiceSignature;

pub(super) fn signature(
    encoder: &mut Encoder,
    signature: &PackagePolicyServiceSignature,
) -> Result<(), PackageReviewEncodingError> {
    encoder.sequence(&signature.schema_arguments, encode_type_identity)?;
    encoder.u32(signature.schema_lifetime_parameter_count);
    encoder.sequence(&signature.requirement_arguments, encode_type_identity)?;
    encoder.sequence(
        &signature.requirement_lifetime_arguments,
        |encoder, ordinal| {
            encoder.u32(*ordinal);
            Ok(())
        },
    )?;
    encoder.u32(signature.requirement_lifetime_parameter_count);
    encoder.sequence(&signature.static_parameters, encode_type_parameter)?;
    encoder.sequence(&signature.parameters, |encoder, parameter| {
        encoder.string(&parameter.name)?;
        encode_type_identity(encoder, &parameter.type_identity)?;
        encoder.boolean(parameter.is_const);
        encoder.boolean(parameter.is_mutable);
        encoder.boolean(parameter.is_self);
        Ok(())
    })?;
    encoder.option(signature.result.as_ref(), encode_type_identity)
}
