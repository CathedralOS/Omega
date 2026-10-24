use super::{Encoder, PackageReviewEncodingError};
use crate::encoding::encode::declarations::encode_type_identity;
use crate::encoding::encode::public_api::formal;
use crate::encoding::encode::public_api::type_parameter as encode_type_parameter;
use crate::record::PackagePolicyServiceSignature;

pub(super) fn signature(
    encoder: &mut Encoder,
    signature: &PackagePolicyServiceSignature,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("schema_arguments", |encoder| {
        encoder.sequence(&signature.schema_arguments, encode_type_identity)
    })?;
    encoder.field("schema_lifetime_parameter_count", |encoder| {
        encoder.u32(signature.schema_lifetime_parameter_count);
        Ok(())
    })?;
    encoder.field("schema_lifetime_arguments", |encoder| {
        encoder.sequence(&signature.schema_lifetime_arguments, |encoder, ordinal| {
            encoder.u32(*ordinal);
            Ok(())
        })
    })?;
    encoder.field("application_lifetime_parameter_count", |encoder| {
        encoder.u32(signature.application_lifetime_parameter_count);
        Ok(())
    })?;
    encoder.field("requirement_arguments", |encoder| {
        encoder.sequence(&signature.requirement_arguments, encode_type_identity)
    })?;
    encoder.field("requirement_lifetime_arguments", |encoder| {
        encoder.sequence(
            &signature.requirement_lifetime_arguments,
            |encoder, ordinal| {
                encoder.field("ordinal", |encoder| {
                    encoder.u32(*ordinal);
                    Ok(())
                })?;
                Ok(())
            },
        )
    })?;
    encoder.field("requirement_lifetime_parameter_count", |encoder| {
        encoder.u32(signature.requirement_lifetime_parameter_count);
        Ok(())
    })?;
    encoder.field("static_parameters", |encoder| {
        encoder.sequence(&signature.static_parameters, encode_type_parameter)
    })?;
    encoder.field("parameters", |encoder| {
        encoder.sequence(&signature.parameters, |encoder, parameter| {
            formal(
                encoder,
                &parameter.name,
                &parameter.type_identity,
                parameter.is_const,
                parameter.is_mutable,
            )
        })
    })?;
    encoder.field("result", |encoder| {
        encoder.option(signature.result.as_ref(), encode_type_identity)
    })
}
