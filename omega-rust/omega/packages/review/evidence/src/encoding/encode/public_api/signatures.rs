use super::super::{
    callable_policy::{crash_route, termination},
    declarations::{encode_data_properties, encode_type_identity},
    values::{
        contracts::encode_callable_contract, effects::encode_synchronous_invocation,
        identity::encode_nominal,
    },
};
use super::*;

pub(in crate::encoding::encode) fn type_parameter(
    encoder: &mut Encoder,
    parameter: &PackagePolicyTypeParameter,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("kind", |encoder| {
        match &parameter.kind {
            PackagePolicyTypeParameterKind::Type => encoder.tag("type", 0),
            PackagePolicyTypeParameterKind::Const(value) => {
                encoder.tag("const", 1);
                encoder.field("type_identity", |encoder| {
                    encode_type_identity(encoder, value)
                })?;
            }
            PackagePolicyTypeParameterKind::Machine(value) => {
                encoder.tag("machine", 2);
                encoder.field("contract", |encoder| machine_contract(encoder, value))?;
            }
            PackagePolicyTypeParameterKind::Proposition(value) => {
                encoder.tag("proposition", 3);
                encoder.field("parameters", |encoder| {
                    encoder.sequence(&value.parameters, |encoder, parameter| {
                        encoder.field("type_identity", |encoder| {
                            encode_type_identity(encoder, &parameter.type_identity)
                        })
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
            encoder.field("parameters", |encoder| {
                encoder.sequence(&signature.parameters, |encoder, parameter| {
                    formal(
                        encoder,
                        &parameter.name,
                        &parameter.type_identity,
                        parameter.is_const,
                        parameter.is_mutable,
                        parameter.is_self,
                    )
                })
            })?;
            encoder.field("return_type", |encoder| {
                encoder.option(signature.return_type.as_ref(), encode_type_identity)
            })?;
            encoder.field("contracts", |encoder| {
                encoder.sequence(&signature.contracts, encode_callable_contract)
            })?;
            encoder.field("published_crash", |encoder| {
                encoder.sequence(&signature.published_crash, crash_route)
            })?;
            encoder.field("service_reach", |encoder| {
                encoder.sequence(&signature.service_reach, encode_nominal)
            })?;
            encoder.field("service_reach_is_installation_bound", |encoder| {
                encoder.boolean(signature.service_reach_is_installation_bound);
                Ok(())
            })?;
            encoder.field("synchronous_invocations", |encoder| {
                encoder.sequence(
                    &signature.synchronous_invocations,
                    encode_synchronous_invocation,
                )
            })?;
            encoder.field("suspends", |encoder| {
                encoder.boolean(signature.suspends);
                Ok(())
            })?;
            encoder.field("blocks", |encoder| {
                encoder.boolean(signature.blocks);
                Ok(())
            })?;
            encoder.field("termination", |encoder| {
                termination(encoder, &signature.termination)
            })
        }
        PackagePolicyMachineParameterContract::Nominal {
            trait_identity,
            requirement_identity,
        } => {
            encoder.tag("nominal", 1);
            encoder.field("trait_identity", |encoder| {
                encode_nominal(encoder, trait_identity)
            })?;
            encoder.field("requirement_identity", |encoder| {
                encode_nominal(encoder, requirement_identity)
            })
        }
        PackagePolicyMachineParameterContract::RequirementIdentity => {
            encoder.tag("requirement_identity", 2);
            Ok(())
        }
    })
}

pub(super) fn formal(
    encoder: &mut Encoder,
    name: &str,
    value: &PackageReviewTypeIdentity,
    is_const: bool,
    is_mutable: bool,
    is_self: bool,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("name", |encoder| encoder.string(name))?;
    encoder.field("type_identity", |encoder| {
        encode_type_identity(encoder, value)
    })?;
    encoder.field("is_const", |encoder| {
        encoder.boolean(is_const);
        Ok(())
    })?;
    encoder.field("is_mutable", |encoder| {
        encoder.boolean(is_mutable);
        Ok(())
    })?;
    encoder.field("is_self", |encoder| {
        encoder.boolean(is_self);
        Ok(())
    })?;
    Ok(())
}
