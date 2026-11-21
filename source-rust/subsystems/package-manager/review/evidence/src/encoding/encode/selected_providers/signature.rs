// Canonical encoding for selected-provider service signatures, schema arguments, and formals.

use super::{Encoder, PackageReviewEncodingError};
use crate::encoding::encode::declarations::encode_type_identity;
use crate::encoding::encode::public_api::formal;
use crate::encoding::encode::public_api::type_parameter as encode_type_parameter;
use crate::record::PackagePolicyServiceSignature;

pub(super) fn signature(
    encoder: &mut Encoder,
    signature: &PackagePolicyServiceSignature,
) -> Result<(), PackageReviewEncodingError> {
    let lifetime_count = signature
        .schema_lifetime_parameter_count
        .checked_add(signature.application_lifetime_parameter_count)
        .and_then(|count| count.checked_add(signature.requirement_lifetime_parameter_count))
        .ok_or_else(|| {
            PackageReviewEncodingError::new("selected service lifetime scope overflows")
        })? as usize;
    encoder.field("schema_arguments", |encoder| {
        encoder.sequence(&signature.schema_arguments, Encoder::static_argument)
    })?;
    encoder.u32_field(
        "schema_lifetime_parameter_count",
        signature.schema_lifetime_parameter_count,
    )?;
    encoder.field("schema_lifetime_arguments", |encoder| {
        encoder.sequence(&signature.schema_lifetime_arguments, |encoder, ordinal| {
            encoder.u32(*ordinal);
            Ok(())
        })
    })?;
    encoder.u32_field(
        "application_lifetime_parameter_count",
        signature.application_lifetime_parameter_count,
    )?;
    encoder.field("requirement_arguments", |encoder| {
        encoder.sequence(&signature.requirement_arguments, Encoder::static_argument)
    })?;
    encoder.field("requirement_lifetime_arguments", |encoder| {
        encoder.sequence(
            &signature.requirement_lifetime_arguments,
            |encoder, ordinal| {
                encoder.u32_field("ordinal", *ordinal)?;
                Ok(())
            },
        )
    })?;
    encoder.u32_field(
        "requirement_lifetime_parameter_count",
        signature.requirement_lifetime_parameter_count,
    )?;
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
                lifetime_count,
            )
        })
    })?;
    encoder.option_field("result", signature.result.as_ref(), |encoder, identity| {
        super::super::type_identity_in_scope(encoder, identity, Some(lifetime_count))
    })
}
