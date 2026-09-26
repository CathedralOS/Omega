use super::{Encoder, PackageReviewEncodingError};
use crate::encoding::encode::declarations::encode_type_identity;
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
            encoder.field("name", |encoder| encoder.string(&parameter.name))?;
            encoder.field("type_identity", |encoder| {
                encode_type_identity(encoder, &parameter.type_identity)
            })?;
            encoder.field("is_const", |encoder| {
                encoder.boolean(parameter.is_const);
                Ok(())
            })?;
            encoder.field("is_mutable", |encoder| {
                encoder.boolean(parameter.is_mutable);
                Ok(())
            })?;
            encoder.field("is_self", |encoder| {
                encoder.boolean(parameter.is_self);
                Ok(())
            })?;
            Ok(())
        })
    })?;
    encoder.field("result", |encoder| {
        encoder.option(signature.result.as_ref(), encode_type_identity)
    })
}
