use super::super::declarations::{
    encode_conformance_bound, encode_data_properties, encode_type_identity, encode_type_parameter,
};
use super::super::encoder::Encoder;
use crate::encoding::PackageReviewEncodingError;
use crate::record::{
    CheckedPackageCallableReview, PackageReviewCallableConformance, PackageReviewCallableRole,
    PackageReviewCheckedServiceReach, PackageReviewExternalBinding,
    PackageReviewExternalCallableSignature, PackageReviewExternalExecutableSupply,
    PackageReviewExternalRequirement, PackageReviewExternalStaticParameter,
};

use super::contracts::encode_callable_contract;
use super::crashes::encode_crash;
use super::declarations::encode_operator_coordinate;
use super::effects::{
    encode_capability_flow, encode_installation_reach, encode_mutation,
    encode_synchronous_invocation, encode_termination,
};
use super::identity::{encode_nominal, encode_supply};

pub(crate) fn encode_callable(
    encoder: &mut Encoder,
    callable: &CheckedPackageCallableReview,
) -> Result<(), PackageReviewEncodingError> {
    encoder.byte(match callable.role {
        PackageReviewCallableRole::Boundary => 0,
        PackageReviewCallableRole::Public => 1,
        PackageReviewCallableRole::Build => 2,
    });
    encode_nominal(encoder, &callable.identity)?;
    encode_supply(encoder, callable.supply)?;
    encoder.usize(callable.lifetime_parameter_count)?;
    encoder.sequence(&callable.type_parameters, encode_type_parameter)?;
    encoder.sequence(&callable.conformance_bounds, encode_conformance_bound)?;
    encoder.sequence(&callable.parameters, |encoder, parameter| {
        encoder.string(&parameter.name)?;
        encode_type_identity(encoder, &parameter.type_identity)?;
        encoder.boolean(parameter.is_const);
        encoder.boolean(parameter.is_mutable);
        encoder.boolean(parameter.is_self);
        Ok(())
    })?;
    encode_type_identity(encoder, &callable.return_type)?;
    encoder.sequence(&callable.conformances, encode_callable_conformance)?;
    encoder.sequence(&callable.operator_realizations, |encoder, realization| {
        encode_operator_coordinate(encoder, &realization.coordinate)?;
        encoder.option(realization.alias.as_deref(), |encoder, alias| {
            encoder.string(alias)
        })
    })?;
    encoder.sequence(&callable.contracts, encode_callable_contract)?;
    encoder.option(
        callable.declared_service_reach.as_deref(),
        |encoder, row| encoder.sequence(row, encode_nominal),
    )?;
    match &callable.checked_service_reach {
        PackageReviewCheckedServiceReach::NoCheckedBody => encoder.byte(0),
        PackageReviewCheckedServiceReach::CheckedBody { realized, concrete } => {
            encoder.byte(1);
            encoder.sequence(realized, encode_nominal)?;
            encoder.sequence(concrete, encode_nominal)?;
        }
    }
    encoder.sequence(
        &callable.unresolved_installation_reaches,
        encode_installation_reach,
    )?;
    encoder.option(
        callable.declared_synchronous_invocations.as_deref(),
        |encoder, invocations| encoder.sequence(invocations, encode_synchronous_invocation),
    )?;
    encoder.sequence(
        &callable.realized_synchronous_invocations,
        encode_synchronous_invocation,
    )?;
    encoder.sequence(&callable.capability_flows, encode_capability_flow)?;
    encoder.boolean(callable.checked_may_suspend);
    encoder.boolean(callable.checked_may_block);
    encode_termination(encoder, &callable.checked_termination)?;
    encode_crash(encoder, &callable.checked_crash)?;
    encoder.sequence(&callable.mutation, encode_mutation)
}

pub(crate) fn encode_callable_conformance(
    encoder: &mut Encoder,
    conformance: &PackageReviewCallableConformance,
) -> Result<(), PackageReviewEncodingError> {
    encode_nominal(encoder, &conformance.trait_identity)?;
    encode_nominal(encoder, &conformance.requirement_identity)?;
    encoder.sequence(&conformance.arguments, encode_type_identity)?;
    encoder.option(conformance.alias.as_deref(), |encoder, alias| {
        encoder.string(alias)
    })
}

pub(crate) fn encode_external_executable_supply_key(
    encoder: &mut Encoder,
    supply: &PackageReviewExternalExecutableSupply,
) -> Result<(), PackageReviewEncodingError> {
    encode_nominal(encoder, &supply.callable)?;
    encode_external_callable_signature(encoder, &supply.signature)?;
    match &supply.requirement {
        PackageReviewExternalRequirement::Trait(conformance) => {
            encoder.byte(0);
            encode_callable_conformance(encoder, conformance)
        }
        PackageReviewExternalRequirement::Operator(operator) => {
            encoder.byte(1);
            encode_operator_coordinate(encoder, operator)
        }
        PackageReviewExternalRequirement::TopLevelRequirement {
            identity,
            signature,
        } => {
            encoder.byte(2);
            encode_nominal(encoder, identity)?;
            encode_external_callable_signature(encoder, signature)
        }
    }
}

pub(super) fn encode_external_callable_signature(
    encoder: &mut Encoder,
    signature: &PackageReviewExternalCallableSignature,
) -> Result<(), PackageReviewEncodingError> {
    encoder.usize(signature.lifetime_parameter_count)?;
    encoder.sequence(&signature.static_parameters, |encoder, parameter| {
        match parameter {
            PackageReviewExternalStaticParameter::Type { properties } => {
                encoder.byte(0);
                encode_data_properties(encoder, *properties);
            }
        }
        Ok(())
    })?;
    encoder.sequence(&signature.parameters, |encoder, parameter| {
        encode_type_identity(encoder, &parameter.type_identity)?;
        encoder.boolean(parameter.is_const);
        encoder.boolean(parameter.is_mutable);
        encoder.boolean(parameter.is_self);
        Ok(())
    })?;
    encode_type_identity(encoder, &signature.return_type)
}

pub(crate) fn encode_external_executable_supply(
    encoder: &mut Encoder,
    supply: &PackageReviewExternalExecutableSupply,
) -> Result<(), PackageReviewEncodingError> {
    encode_external_executable_supply_key(encoder, supply)?;
    match &supply.binding {
        PackageReviewExternalBinding::Import { library, symbol } => {
            encoder.byte(0);
            encoder.string(library)?;
            encoder.string(symbol)?;
        }
        PackageReviewExternalBinding::Syscall { number } => {
            encoder.byte(1);
            encoder.i64(*number);
        }
        PackageReviewExternalBinding::CompilerIntrinsic => encoder.byte(2),
        PackageReviewExternalBinding::VtableSlot { index } => {
            encoder.byte(3);
            encoder.i64(*index);
        }
        PackageReviewExternalBinding::VtableField { field } => {
            encoder.byte(4);
            encoder.string(field)?;
        }
        PackageReviewExternalBinding::TableFunction { field } => {
            encoder.byte(5);
            encoder.string(field)?;
        }
    }
    Ok(())
}
