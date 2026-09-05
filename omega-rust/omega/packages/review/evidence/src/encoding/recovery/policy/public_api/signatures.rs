use super::super::{
    Error,
    behavior::synchronous_invocation,
    callable_policy::{crash_route, termination},
    contracts::callable_contract,
    identity::{nominal, type_identity},
    reader::Reader,
    signatures::data_properties,
};
use crate::record::*;

pub(in crate::encoding::recovery::policy) fn type_parameter(
    reader: &mut Reader<'_>,
) -> Result<PackagePolicyTypeParameter, Error> {
    let kind = match reader.byte()? {
        0 => PackagePolicyTypeParameterKind::Type,
        1 => PackagePolicyTypeParameterKind::Const(type_identity(reader)?),
        2 => PackagePolicyTypeParameterKind::Machine(machine_contract(reader)?),
        3 => PackagePolicyTypeParameterKind::Proposition(
            PackageReviewPropositionParameterSignature {
                parameters: reader.sequence(8, |reader| {
                    Ok(PackageReviewPropositionParameterValue {
                        type_identity: type_identity(reader)?,
                    })
                })?,
            },
        ),
        _ => return Err(Error::InvalidTag),
    };
    Ok(PackagePolicyTypeParameter {
        kind,
        bounds: data_properties(reader)?,
    })
}

pub(in crate::encoding::recovery::policy) fn machine_contract(
    reader: &mut Reader<'_>,
) -> Result<PackagePolicyMachineParameterContract, Error> {
    reader.nested(|reader| {
        Ok(match reader.byte()? {
            0 => PackagePolicyMachineParameterContract::Structural(machine_signature(reader)?),
            1 => PackagePolicyMachineParameterContract::Nominal {
                trait_identity: nominal(reader)?,
                requirement_identity: nominal(reader)?,
            },
            2 => PackagePolicyMachineParameterContract::RequirementIdentity,
            _ => return Err(Error::InvalidTag),
        })
    })
}

fn machine_signature(
    reader: &mut Reader<'_>,
) -> Result<PackagePolicyMachineParameterSignature, Error> {
    Ok(PackagePolicyMachineParameterSignature {
        lifetime_parameter_count: reader.usize()?,
        type_parameters: reader.sequence(3, type_parameter)?,
        parameters: reader.sequence(19, |reader| {
            Ok(PackageReviewMachineParameterValue {
                name: reader.string()?,
                type_identity: type_identity(reader)?,
                is_const: reader.boolean()?,
                is_mutable: reader.boolean()?,
                is_self: reader.boolean()?,
            })
        })?,
        return_type: reader.option(type_identity)?,
        contracts: reader.sequence(4, callable_contract)?,
        published_crash: reader.sequence(9, crash_route)?,
        service_reach: reader.sequence(41, nominal)?,
        service_reach_is_installation_bound: reader.boolean()?,
        synchronous_invocations: reader.sequence(5, synchronous_invocation)?,
        suspends: reader.boolean()?,
        blocks: reader.boolean()?,
        termination: termination(reader)?,
    })
}
