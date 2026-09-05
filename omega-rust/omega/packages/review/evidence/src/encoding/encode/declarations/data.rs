use super::super::PackageReviewEncodingError;
use super::super::encoder::Encoder;
use super::super::values::contracts::{encode_callable_contract, encode_contract_fact};
use super::super::values::crashes::encode_crash_route;
use super::super::values::effects::{encode_synchronous_invocation, encode_termination};
use super::super::values::identity::encode_nominal;
use crate::record::{
    PackageReviewDataField, PackageReviewDataKind, PackageReviewDataMember,
    PackageReviewDataProperties, PackageReviewDataShape, PackageReviewMachineParameterContract,
    PackageReviewMachineParameterSignature, PackageReviewTypeIdentity, PackageReviewTypeParameter,
    PackageReviewTypeParameterKind,
};

pub(crate) fn encode_data_shape(
    encoder: &mut Encoder,
    shape: &PackageReviewDataShape,
) -> Result<(), PackageReviewEncodingError> {
    encode_nominal(encoder, &shape.identity)?;
    match &shape.kind {
        PackageReviewDataKind::Ordinary => encoder.byte(0),
        PackageReviewDataKind::Quotient { carrier, relation } => {
            encoder.byte(1);
            encode_type_identity(encoder, carrier)?;
            encode_nominal(encoder, relation)?;
        }
    }
    encoder.byte(match shape.supply {
        psi_language_semantics::DataSupplyMode::CheckedShape => 0,
        psi_language_semantics::DataSupplyMode::BoundaryOpaque => 1,
    });
    encoder.usize(shape.lifetime_parameter_count)?;
    encoder.sequence(&shape.type_parameters, encode_type_parameter)?;
    encode_data_properties(encoder, shape.properties);
    encoder.boolean(shape.zero_gated);
    encoder.sequence(&shape.invariants, encode_contract_fact)?;
    encoder.sequence(&shape.retired_identities, |encoder, identity| {
        encoder.u64(*identity);
        Ok(())
    })?;
    encoder.sequence(&shape.members, encode_data_member)
}

pub(crate) fn encode_type_parameter(
    encoder: &mut Encoder,
    parameter: &PackageReviewTypeParameter,
) -> Result<(), PackageReviewEncodingError> {
    match &parameter.kind {
        PackageReviewTypeParameterKind::Type => encoder.byte(0),
        PackageReviewTypeParameterKind::Const(type_identity) => {
            encoder.byte(1);
            encode_type_identity(encoder, type_identity)?;
        }
        PackageReviewTypeParameterKind::Machine(contract) => {
            encoder.byte(2);
            encode_machine_parameter_contract(encoder, contract)?;
        }
        PackageReviewTypeParameterKind::Proposition(signature) => {
            encoder.byte(3);
            encoder.sequence(&signature.parameters, |encoder, parameter| {
                encode_type_identity(encoder, &parameter.type_identity)
            })?;
        }
    }
    encode_data_properties(encoder, parameter.bounds);
    Ok(())
}

pub(crate) fn encode_machine_parameter_contract(
    encoder: &mut Encoder,
    contract: &PackageReviewMachineParameterContract,
) -> Result<(), PackageReviewEncodingError> {
    encoder.nested(|encoder| match contract {
        PackageReviewMachineParameterContract::Structural(signature) => {
            encoder.byte(0);
            encode_machine_parameter_signature(encoder, signature)
        }
        PackageReviewMachineParameterContract::Nominal {
            trait_identity,
            requirement_identity,
        } => {
            encoder.byte(1);
            encode_nominal(encoder, trait_identity)?;
            encode_nominal(encoder, requirement_identity)
        }
        PackageReviewMachineParameterContract::RequirementIdentity => {
            encoder.byte(2);
            Ok(())
        }
    })
}

pub(crate) fn encode_machine_parameter_signature(
    encoder: &mut Encoder,
    signature: &PackageReviewMachineParameterSignature,
) -> Result<(), PackageReviewEncodingError> {
    encoder.usize(signature.lifetime_parameter_count)?;
    encoder.sequence(&signature.type_parameters, encode_type_parameter)?;
    encoder.sequence(&signature.parameters, |encoder, parameter| {
        encoder.string(&parameter.name)?;
        encode_type_identity(encoder, &parameter.type_identity)?;
        encoder.boolean(parameter.is_const);
        encoder.boolean(parameter.is_mutable);
        encoder.boolean(parameter.is_self);
        Ok(())
    })?;
    encode_type_identity(encoder, &signature.return_type)?;
    encoder.sequence(&signature.contracts, encode_callable_contract)?;
    encoder.sequence(&signature.published_crash, encode_crash_route)?;
    encoder.sequence(&signature.service_reach, encode_nominal)?;
    encoder.boolean(signature.service_reach_is_installation_bound);
    encoder.sequence(
        &signature.synchronous_invocations,
        encode_synchronous_invocation,
    )?;
    encoder.boolean(signature.suspends);
    encoder.boolean(signature.blocks);
    encode_termination(encoder, &signature.termination)
}

pub(crate) fn encode_data_properties(
    encoder: &mut Encoder,
    properties: PackageReviewDataProperties,
) {
    encoder.byte(match properties.multiplicity {
        psi_language_semantics::Multiplicity::Unrestricted => 0,
        psi_language_semantics::Multiplicity::Affine => 1,
        psi_language_semantics::Multiplicity::Linear => 2,
    });
    match properties.carry {
        None => encoder.byte(0),
        Some(carry) => {
            encoder.byte(1);
            encoder.byte(match carry.suspension {
                psi_language_semantics::CarrySuspension::Forbidden => 0,
                psi_language_semantics::CarrySuspension::Allowed => 1,
            });
            encoder.byte(match carry.cpu {
                psi_language_semantics::CarryCpu::Origin => 0,
                psi_language_semantics::CarryCpu::Any => 1,
            });
            encoder.byte(match carry.host_thread {
                psi_language_semantics::CarryHostThread::Origin => 0,
                psi_language_semantics::CarryHostThread::Any => 1,
            });
            encoder.byte(match carry.address {
                psi_language_semantics::CarryAddress::Stable => 0,
                psi_language_semantics::CarryAddress::Movable => 1,
            });
        }
    }
}

pub(crate) fn encode_data_member(
    encoder: &mut Encoder,
    member: &PackageReviewDataMember,
) -> Result<(), PackageReviewEncodingError> {
    match member {
        PackageReviewDataMember::Field(field) => {
            encoder.byte(0);
            encode_data_field(encoder, field)?;
        }
        PackageReviewDataMember::Variant {
            identity,
            name,
            payload,
            retired_payload_identities,
        } => {
            encoder.byte(1);
            encode_optional_u64(encoder, *identity);
            encoder.string(name)?;
            encoder.sequence(payload, encode_data_field)?;
            encoder.sequence(retired_payload_identities, |encoder, identity| {
                encoder.u64(*identity);
                Ok(())
            })?;
        }
    }
    Ok(())
}

pub(crate) fn encode_data_field(
    encoder: &mut Encoder,
    field: &PackageReviewDataField,
) -> Result<(), PackageReviewEncodingError> {
    encode_optional_u64(encoder, field.identity);
    encoder.string(&field.name)?;
    encode_relevance(encoder, field.relevance);
    encode_type_identity(encoder, &field.type_identity)
}

pub(crate) fn encode_type_identity(
    encoder: &mut Encoder,
    identity: &PackageReviewTypeIdentity,
) -> Result<(), PackageReviewEncodingError> {
    encoder.string(&identity.canonical)
}

pub(crate) fn encode_relevance(
    encoder: &mut Encoder,
    relevance: psi_language_core::BindingRelevance,
) {
    encoder.byte(match relevance {
        psi_language_core::BindingRelevance::Relevant => 0,
        psi_language_core::BindingRelevance::Erased => 1,
    });
}

pub(crate) fn encode_optional_u64(encoder: &mut Encoder, value: Option<u64>) {
    match value {
        None => encoder.byte(0),
        Some(value) => {
            encoder.byte(1);
            encoder.u64(value);
        }
    }
}
