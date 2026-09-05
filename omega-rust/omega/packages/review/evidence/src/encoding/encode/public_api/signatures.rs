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
    match &parameter.kind {
        PackagePolicyTypeParameterKind::Type => encoder.byte(0),
        PackagePolicyTypeParameterKind::Const(value) => {
            encoder.byte(1);
            encode_type_identity(encoder, value)?;
        }
        PackagePolicyTypeParameterKind::Machine(value) => {
            encoder.byte(2);
            machine_contract(encoder, value)?;
        }
        PackagePolicyTypeParameterKind::Proposition(value) => {
            encoder.byte(3);
            encoder.sequence(&value.parameters, |encoder, parameter| {
                encode_type_identity(encoder, &parameter.type_identity)
            })?;
        }
    }
    encode_data_properties(encoder, parameter.bounds);
    Ok(())
}

pub(in crate::encoding) fn machine_contract(
    encoder: &mut Encoder,
    contract: &PackagePolicyMachineParameterContract,
) -> Result<(), PackageReviewEncodingError> {
    encoder.nested(|encoder| match contract {
        PackagePolicyMachineParameterContract::Structural(signature) => {
            encoder.byte(0);
            encoder.usize(signature.lifetime_parameter_count)?;
            encoder.sequence(&signature.type_parameters, type_parameter)?;
            encoder.sequence(&signature.parameters, |encoder, parameter| {
                formal(
                    encoder,
                    &parameter.name,
                    &parameter.type_identity,
                    parameter.is_const,
                    parameter.is_mutable,
                    parameter.is_self,
                )
            })?;
            encoder.option(signature.return_type.as_ref(), encode_type_identity)?;
            encoder.sequence(&signature.contracts, encode_callable_contract)?;
            encoder.sequence(&signature.published_crash, crash_route)?;
            encoder.sequence(&signature.service_reach, encode_nominal)?;
            encoder.boolean(signature.service_reach_is_installation_bound);
            encoder.sequence(
                &signature.synchronous_invocations,
                encode_synchronous_invocation,
            )?;
            encoder.boolean(signature.suspends);
            encoder.boolean(signature.blocks);
            termination(encoder, &signature.termination)
        }
        PackagePolicyMachineParameterContract::Nominal {
            trait_identity,
            requirement_identity,
        } => {
            encoder.byte(1);
            encode_nominal(encoder, trait_identity)?;
            encode_nominal(encoder, requirement_identity)
        }
        PackagePolicyMachineParameterContract::RequirementIdentity => {
            encoder.byte(2);
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
    encoder.string(name)?;
    encode_type_identity(encoder, value)?;
    encoder.boolean(is_const);
    encoder.boolean(is_mutable);
    encoder.boolean(is_self);
    Ok(())
}
