use super::super::{
    callable_policy::{crash_route, termination},
    declarations::{encode_data_properties, encode_type_identity},
    values::{
        contracts::encode_callable_contract, effects::encode_synchronous_invocation,
        identity::encode_nominal,
    },
};
use crate::encoding::PackageReviewEncodingError;
use crate::encoding::encode::public_api::Encoder;
use crate::record::PackagePolicyMachineParameterContract;
use crate::record::PackagePolicyTypeParameter;
use crate::record::PackagePolicyTypeParameterKind;
use crate::record::PackageReviewTypeIdentity;

// The `parameters`+`return_type` field pair every callable-shape encoding
// restates — the carrier differs per record, the encoding does not.
macro_rules! encode_callable_shape {
    ($encoder:expr, $parameters:expr, $return_type:expr) => {
        ($encoder).field("parameters", |encoder| {
            encoder.sequence($parameters, |encoder, parameter| {
                formal(
                    encoder,
                    &parameter.name,
                    &parameter.type_identity,
                    parameter.is_const,
                    parameter.is_mutable,
                )
            })
        })?;
        ($encoder).field("return_type", |encoder| {
            encoder.option($return_type, encode_type_identity)
        })?;
    };
}
pub(crate) use encode_callable_shape;

// The published behavior tail every callable-shape encoding restates —
// contracts, crash routes, service reach with its installation binding,
// synchronous invocations, suspension, blocking, and termination. A service
// requirement and a machine parameter signature carry the same tail under the
// same field names; the carrier record differs, the encoding does not. The
// expansion is the tail expression of the shape's encoder, so it yields the
// last field's result.
macro_rules! encode_callable_behavior_tail {
    ($encoder:expr, $record:ident) => {{
        ($encoder).field("contracts", |encoder| {
            encoder.sequence(&$record.contracts, encode_callable_contract)
        })?;
        ($encoder).field("published_crash", |encoder| {
            encoder.sequence(&$record.published_crash, crash_route)
        })?;
        ($encoder).field("service_reach", |encoder| {
            encoder.sequence(&$record.service_reach, encode_nominal)
        })?;
        ($encoder).field("service_reach_is_installation_bound", |encoder| {
            encoder.boolean($record.service_reach_is_installation_bound);
            Ok(())
        })?;
        ($encoder).field("synchronous_invocations", |encoder| {
            encoder.sequence(
                &$record.synchronous_invocations,
                encode_synchronous_invocation,
            )
        })?;
        ($encoder).field("suspends", |encoder| {
            encoder.boolean($record.suspends);
            Ok(())
        })?;
        ($encoder).field("blocks", |encoder| {
            encoder.boolean($record.blocks);
            Ok(())
        })?;
        ($encoder).field("termination", |encoder| {
            termination(encoder, &$record.termination)
        })
    }};
}
pub(in crate::encoding) use encode_callable_behavior_tail;

pub(in crate::encoding::encode) fn type_parameter(
    encoder: &mut Encoder,
    parameter: &PackagePolicyTypeParameter,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("kind", |encoder| {
        match &parameter.kind {
            PackagePolicyTypeParameterKind::Type => encoder.tag("type", 0),
            PackagePolicyTypeParameterKind::Implementation => encoder.tag("implementation", 5),
            PackagePolicyTypeParameterKind::Const(value) => {
                encoder.tag("const", 1);
                encoder.field_by("type_identity", value, encode_type_identity)?;
            }
            PackagePolicyTypeParameterKind::Value(value) => {
                encoder.tag("value", 4);
                encoder.field_by("type_identity", value, encode_type_identity)?;
            }
            PackagePolicyTypeParameterKind::Machine(value) => {
                encoder.tag("machine", 2);
                encoder.field("contract", |encoder| machine_contract(encoder, value))?;
            }
            PackagePolicyTypeParameterKind::Proposition(value) => {
                encoder.tag("proposition", 3);
                encoder.field("parameters", |encoder| {
                    encoder.sequence(&value.parameters, |encoder, parameter| {
                        encoder.field_by(
                            "type_identity",
                            &parameter.type_identity,
                            encode_type_identity,
                        )?;
                        encoder.field("is_const", |encoder| {
                            encoder.boolean(parameter.is_const);
                            Ok(())
                        })?;
                        encoder.field("is_mutable", |encoder| {
                            encoder.boolean(parameter.is_mutable);
                            Ok(())
                        })?;
                        Ok(())
                    })
                })?;
            }
        };
        Ok(())
    })?;
    encoder.field("bounds", |encoder| {
        encode_data_properties(encoder, parameter.bounds);
        Ok(())
    })?;
    Ok(())
}

pub(in crate::encoding) fn machine_contract(
    encoder: &mut Encoder,
    contract: &PackagePolicyMachineParameterContract,
) -> Result<(), PackageReviewEncodingError> {
    encoder.nested(|encoder| match contract {
        PackagePolicyMachineParameterContract::Structural(signature) => {
            encoder.tag("structural", 0);
            encoder.field("lifetime_parameter_count", |encoder| {
                encoder.usize(signature.lifetime_parameter_count)
            })?;
            encoder.field("type_parameters", |encoder| {
                encoder.sequence(&signature.type_parameters, type_parameter)
            })?;
            encode_callable_shape!(
                encoder,
                &signature.parameters,
                signature.return_type.as_ref()
            );
            encode_callable_behavior_tail!(encoder, signature)
        }
        PackagePolicyMachineParameterContract::Nominal {
            trait_identity,
            requirement_identity,
            trait_arguments,
        } => {
            encoder.tag("nominal", 1);
            encoder.field_by("trait_identity", trait_identity, encode_nominal)?;
            encoder.field_by("requirement_identity", requirement_identity, encode_nominal)?;
            encoder.field("trait_arguments", |encoder| {
                encoder.sequence(trait_arguments, encode_type_identity)
            })
        }
        PackagePolicyMachineParameterContract::RequirementIdentity => {
            encoder.tag("requirement_identity", 2);
            Ok(())
        }
    })
}

pub(in crate::encoding) fn formal(
    encoder: &mut Encoder,
    name: &str,
    value: &PackageReviewTypeIdentity,
    is_const: bool,
    is_mutable: bool,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("name", |encoder| encoder.string(name))?;
    encoder.field_by("type_identity", value, encode_type_identity)?;
    encoder.field("is_const", |encoder| {
        encoder.boolean(is_const);
        Ok(())
    })?;
    encoder.field("is_mutable", |encoder| {
        encoder.boolean(is_mutable);
        Ok(())
    })?;
    Ok(())
}
