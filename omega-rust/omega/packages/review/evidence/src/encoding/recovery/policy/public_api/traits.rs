use super::super::{
    Error,
    behavior::synchronous_invocation,
    callable_policy::{crash_route, termination},
    contracts::{callable_contract, evidence_interface},
    declarations::{operator_spelling, trait_parent},
    identity::{nominal, operator_coordinate, type_identity},
    reader::Reader,
    signatures::conformance_bound,
};
use super::signatures::type_parameter;
use crate::record::*;

pub(super) fn trait_shape(reader: &mut Reader<'_>) -> Result<PackagePolicyTraitShape, Error> {
    Ok(PackagePolicyTraitShape {
        identity: nominal(reader)?,
        is_boundary: reader.boolean()?,
        lifetime_parameter_count: reader.usize()?,
        type_parameters: reader.sequence(3, type_parameter)?,
        conformance_bounds: reader.sequence(62, conformance_bound)?,
        parents: reader.sequence(58, trait_parent)?,
        requirements: reader.sequence(104, trait_requirement)?,
    })
}

fn trait_requirement(reader: &mut Reader<'_>) -> Result<PackagePolicyTraitRequirement, Error> {
    Ok(PackagePolicyTraitRequirement {
        identity: nominal(reader)?,
        spelling: reader.option(operator_spelling)?,
        has_default_realization: reader.boolean()?,
        lifetime_parameter_count: reader.usize()?,
        type_parameters: reader.sequence(3, type_parameter)?,
        parameters: reader.sequence(19, |reader| {
            Ok(PackageReviewTraitRequirementParameter {
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

pub(in crate::encoding::recovery::policy) fn conformance_shape(
    reader: &mut Reader<'_>,
) -> Result<PackagePolicyConformanceShape, Error> {
    Ok(PackagePolicyConformanceShape {
        identity: nominal(reader)?,
        lifetime_parameter_count: reader.usize()?,
        type_parameters: reader.sequence(3, type_parameter)?,
        subject: match reader.byte()? {
            0 => PackageReviewConformanceSubject::Subjectless,
            1 => PackageReviewConformanceSubject::TypeParameter(reader.u32()?),
            2 => PackageReviewConformanceSubject::Nominal(nominal(reader)?),
            _ => return Err(Error::InvalidTag),
        },
        interface: evidence_interface(reader)?,
    })
}

pub(super) fn operator_shape(reader: &mut Reader<'_>) -> Result<PackagePolicyOperatorShape, Error> {
    Ok(PackagePolicyOperatorShape {
        coordinate: operator_coordinate(reader)?,
        is_boundary: reader.boolean()?,
        spelling: reader.option(operator_spelling)?,
        lifetime_parameter_count: reader.usize()?,
        type_parameters: reader.sequence(3, type_parameter)?,
        parameters: reader.sequence(19, |reader| {
            Ok(PackageReviewCallableParameter {
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
    })
}
