use super::super::{declarations::encode_operator_coordinate, identity::encode_nominal};
use crate::encoding::PackageReviewEncodingError;
use crate::encoding::encode::{
    callable_policy::encode_callable_conformance,
    declarations::{encode_conformance_bound, encode_type_identity},
    encoder::Encoder,
    public_api::type_parameter,
};
use crate::record::*;

pub(super) fn signature(
    encoder: &mut Encoder,
    value: &PackagePolicyExternalCallableSignature,
) -> Result<(), PackageReviewEncodingError> {
    encoder.usize(value.lifetime_parameter_count)?;
    encoder.sequence(&value.static_parameters, type_parameter)?;
    encoder.sequence(&value.conformance_bounds, encode_conformance_bound)?;
    encoder.sequence(&value.parameters, |encoder, parameter| {
        encode_type_identity(encoder, &parameter.type_identity)?;
        encoder.boolean(parameter.is_const);
        encoder.boolean(parameter.is_mutable);
        encoder.boolean(parameter.is_self);
        Ok(())
    })?;
    encoder.option(value.return_type.as_ref(), encode_type_identity)
}

pub(super) fn requirement(
    encoder: &mut Encoder,
    value: &PackagePolicyExternalRequirement,
) -> Result<(), PackageReviewEncodingError> {
    match value {
        PackagePolicyExternalRequirement::Trait(value) => {
            encoder.byte(0);
            encode_callable_conformance(encoder, value)
        }
        PackagePolicyExternalRequirement::Operator { coordinate, alias } => {
            encoder.byte(1);
            encode_operator_coordinate(encoder, coordinate)?;
            encoder.option(alias.as_ref(), |encoder, value| encoder.string(value))
        }
        PackagePolicyExternalRequirement::TopLevelRequirement {
            identity,
            signature: value,
            alias,
        } => {
            encoder.byte(2);
            encode_nominal(encoder, identity)?;
            signature(encoder, value)?;
            encoder.option(alias.as_ref(), |encoder, value| encoder.string(value))
        }
    }
}
