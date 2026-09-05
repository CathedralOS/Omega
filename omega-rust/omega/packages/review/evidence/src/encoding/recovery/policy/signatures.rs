//! Inverse of the complete structural signature vocabulary, without receipts.

use super::Error;
use super::expressions::static_argument;
use super::identity::{nominal, type_identity};
use super::reader::Reader;
#[cfg(test)]
use super::{
    behavior::{crash_route, synchronous_invocation, termination},
    contracts::callable_contract,
};
use crate::record::*;

#[cfg(test)]
pub(super) fn machine_contract(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewMachineParameterContract, Error> {
    reader.nested(|reader| {
        Ok(match reader.byte()? {
            0 => PackageReviewMachineParameterContract::Structural(machine_signature(reader)?),
            1 => PackageReviewMachineParameterContract::Nominal {
                trait_identity: nominal(reader)?,
                requirement_identity: nominal(reader)?,
            },
            2 => PackageReviewMachineParameterContract::RequirementIdentity,
            _ => return Err(Error::InvalidTag),
        })
    })
}

#[cfg(test)]
fn machine_signature(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewMachineParameterSignature, Error> {
    Ok(PackageReviewMachineParameterSignature {
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
        return_type: type_identity(reader)?,
        contracts: reader.sequence(4, callable_contract)?,
        published_crash: reader.sequence(1, crash_route)?,
        service_reach: reader.sequence(1, nominal)?,
        service_reach_is_installation_bound: reader.boolean()?,
        synchronous_invocations: reader.sequence(1, synchronous_invocation)?,
        suspends: reader.boolean()?,
        blocks: reader.boolean()?,
        termination: termination(reader)?,
    })
}

#[cfg(test)]
pub(super) fn type_parameter(reader: &mut Reader<'_>) -> Result<PackageReviewTypeParameter, Error> {
    let kind = match reader.byte()? {
        0 => PackageReviewTypeParameterKind::Type,
        1 => PackageReviewTypeParameterKind::Const(type_identity(reader)?),
        2 => PackageReviewTypeParameterKind::Machine(machine_contract(reader)?),
        3 => PackageReviewTypeParameterKind::Proposition(
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
    Ok(PackageReviewTypeParameter {
        kind,
        bounds: data_properties(reader)?,
    })
}

pub(super) fn conformance_bound(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewConformanceBound, Error> {
    let binder_ordinal = reader.option(|reader| reader.u32())?;
    let subject_parameter = reader.u32()?;
    let (selected_conformance, selected_lifetime_arguments, selected_arguments, selected_subject) =
        match reader.byte()? {
            0 => (None, Vec::new(), Vec::new(), None),
            1 => (
                Some(nominal(reader)?),
                reader.sequence(4, |reader| reader.u32())?,
                reader.sequence(1, static_argument)?,
                Some(static_argument(reader)?),
            ),
            _ => return Err(Error::InvalidTag),
        };
    Ok(PackageReviewConformanceBound {
        binder_ordinal,
        subject_parameter,
        selected_conformance,
        selected_lifetime_arguments,
        selected_arguments,
        selected_subject,
        trait_identity: nominal(reader)?,
        trait_lifetime_arguments: reader.sequence(4, |reader| reader.u32())?,
        arguments: reader.sequence(8, type_identity)?,
    })
}

pub(super) fn data_properties(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewDataProperties, Error> {
    use psi_language_semantics::{
        CarryAddress, CarryCpu, CarryHostThread, CarryPolicy, CarrySuspension, Multiplicity,
    };
    let multiplicity = match reader.byte()? {
        0 => Multiplicity::Unrestricted,
        1 => Multiplicity::Affine,
        2 => Multiplicity::Linear,
        _ => return Err(Error::InvalidTag),
    };
    let carry = reader.option(|reader| {
        Ok(CarryPolicy {
            suspension: match reader.byte()? {
                0 => CarrySuspension::Forbidden,
                1 => CarrySuspension::Allowed,
                _ => return Err(Error::InvalidTag),
            },
            cpu: match reader.byte()? {
                0 => CarryCpu::Origin,
                1 => CarryCpu::Any,
                _ => return Err(Error::InvalidTag),
            },
            host_thread: match reader.byte()? {
                0 => CarryHostThread::Origin,
                1 => CarryHostThread::Any,
                _ => return Err(Error::InvalidTag),
            },
            address: match reader.byte()? {
                0 => CarryAddress::Stable,
                1 => CarryAddress::Movable,
                _ => return Err(Error::InvalidTag),
            },
        })
    })?;
    Ok(PackageReviewDataProperties {
        multiplicity,
        carry,
    })
}
