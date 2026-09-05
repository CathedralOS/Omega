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
    encoder.field("lifetime_parameter_count", |encoder| {
        encoder.usize(value.lifetime_parameter_count)
    })?;
    encoder.field("static_parameters", |encoder| {
        encoder.sequence(&value.static_parameters, type_parameter)
    })?;
    encoder.field("conformance_bounds", |encoder| {
        encoder.sequence(&value.conformance_bounds, encode_conformance_bound)
    })?;
    encoder.field("parameters", |encoder| {
        encoder.sequence(&value.parameters, |encoder, parameter| {
            encoder.field("type", |encoder| {
                encode_type_identity(encoder, &parameter.type_identity)
            })?;
            encoder.field("const", |encoder| {
                encoder.boolean(parameter.is_const);
                Ok(())
            })?;
            encoder.field("mutable", |encoder| {
                encoder.boolean(parameter.is_mutable);
                Ok(())
            })?;
            encoder.field("self", |encoder| {
                encoder.boolean(parameter.is_self);
                Ok(())
            })
        })
    })?;
    encoder.field("return_type", |encoder| {
        encoder.option(value.return_type.as_ref(), encode_type_identity)
    })
}

pub(super) fn requirement(
    encoder: &mut Encoder,
    value: &PackagePolicyExternalRequirement,
) -> Result<(), PackageReviewEncodingError> {
    match value {
        PackagePolicyExternalRequirement::Trait(value) => {
            encoder.tag("trait", 0);
            encoder.field("conformance", |encoder| {
                encode_callable_conformance(encoder, value)
            })
        }
        PackagePolicyExternalRequirement::Operator { coordinate, alias } => {
            encoder.tag("operator", 1);
            encoder.field("coordinate", |encoder| {
                encode_operator_coordinate(encoder, coordinate)
            })?;
            encoder.field("alias", |encoder| {
                encoder.option(alias.as_ref(), |encoder, value| encoder.string(value))
            })
        }
        PackagePolicyExternalRequirement::TopLevelRequirement {
            identity,
            signature: value,
            alias,
        } => {
            encoder.tag("top_level_requirement", 2);
            encoder.field("identity", |encoder| encode_nominal(encoder, identity))?;
            encoder.field("signature", |encoder| signature(encoder, value))?;
            encoder.field("alias", |encoder| {
                encoder.option(alias.as_ref(), |encoder, value| encoder.string(value))
            })
        }
    }
}
