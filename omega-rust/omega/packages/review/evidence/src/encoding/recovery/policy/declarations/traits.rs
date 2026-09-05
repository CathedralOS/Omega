use super::super::{
    Error,
    identity::{nominal, type_identity},
    reader::Reader,
};
#[cfg(test)]
use super::super::{
    behavior::{crash_route, synchronous_invocation, termination},
    contracts::{callable_contract, evidence_interface},
    signatures::{conformance_bound, type_parameter},
};
#[cfg(test)]
use super::values::operator_spelling;
use crate::record::*;

#[cfg(test)]
pub(in crate::encoding::recovery::policy) fn trait_shape(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewTraitShape, Error> {
    Ok(PackageReviewTraitShape {
        identity: nominal(reader)?,
        is_boundary: reader.boolean()?,
        lifetime_parameter_count: reader.usize()?,
        type_parameters: reader.sequence(3, type_parameter)?,
        conformance_bounds: reader.sequence(62, conformance_bound)?,
        parents: reader.sequence(58, trait_parent)?,
        requirements: reader.sequence(111, trait_requirement)?,
    })
}

pub(in crate::encoding::recovery::policy) fn trait_parent(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewTraitParent, Error> {
    Ok(PackageReviewTraitParent {
        kind: match reader.byte()? {
            0 => PackageReviewTraitCompositionKind::Policy,
            1 => PackageReviewTraitCompositionKind::ServiceReach,
            _ => return Err(Error::InvalidTag),
        },
        identity: nominal(reader)?,
        lifetime_arguments: reader.sequence(4, |reader| reader.u32())?,
        arguments: reader.sequence(8, type_identity)?,
    })
}

#[cfg(test)]
fn trait_requirement(reader: &mut Reader<'_>) -> Result<PackageReviewTraitRequirement, Error> {
    Ok(PackageReviewTraitRequirement {
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
        return_type: type_identity(reader)?,
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

#[cfg(test)]
pub(in crate::encoding::recovery::policy) fn conformance_shape(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewConformanceShape, Error> {
    Ok(PackageReviewConformanceShape {
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
