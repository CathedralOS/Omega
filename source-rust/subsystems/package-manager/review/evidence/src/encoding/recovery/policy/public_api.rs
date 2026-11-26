// Normalized public API meaning recovery sharing its enclosing baseline budget.

use super::behavior::synchronous_invocation;
use super::callable_policy::{crash_route, termination};
use super::contracts::{callable_contract, contract_fact, evidence_interface};
use super::declarations::{
    alias_atom, data_member, establishment_route, operator_spelling, trait_parent,
};
use super::identity::{nominal, operator_coordinate, signature_parameter, type_identity};
use super::reader::Reader;
use super::signatures::{conformance_bound, data_properties};
use super::{Error, declarations};
use crate::encoding::recovery::policy::identity::service;
use crate::record::{
    PackagePolicyConformanceShape, PackagePolicyDataShape, PackagePolicyDomainShape,
    PackagePolicyMachineParameterContract, PackagePolicyMachineParameterSignature,
    PackagePolicyOperatorShape, PackagePolicyPublicApi, PackagePolicyTraitRequirement,
    PackagePolicyTraitShape, PackagePolicyTypeParameter, PackagePolicyTypeParameterKind,
    PackageReviewBorrowOrigin, PackageReviewDataKind, PackageReviewDomainClassification,
    PackageReviewDomainSemanticRole, PackageReviewParameterQualification,
    PackageReviewPropositionParameterSignature, PackageReviewPropositionParameterValue,
    PackageReviewReturnedBorrowOrigin,
};

pub(in crate::encoding::recovery) fn public_api(
    reader: &mut Reader<'_>,
    schema_version: u16,
) -> Result<PackagePolicyPublicApi, Error> {
    Ok(PackagePolicyPublicApi {
        traits: reader.sequence(1, |reader| trait_shape(reader, schema_version))?,
        conformances: reader.sequence(1, conformance_shape)?,
        domains: reader.sequence(1, domain_shape)?,
        propositions: reader.sequence(1, declarations::proposition_shape)?,
        consts: reader.sequence(1, declarations::const_shape)?,
        operators: reader.sequence(1, operator_shape)?,
        data: reader.sequence(1, data_shape)?,
        type_aliases: if schema_version >= crate::encoding::BASELINE_WITH_TYPE_ALIASES {
            Some(reader.sequence(1, |reader| {
                Ok(crate::record::PackagePolicyTypeAlias {
                    identity: super::identity::nominal(reader)?,
                    lifetime_parameter_count: reader.usize()?,
                    type_parameters: reader.sequence(3, type_parameter)?,
                    conformance_bounds: reader
                        .sequence(62, super::signatures::conformance_bound)?,
                    expansion: super::identity::type_identity(reader)?,
                })
            })?)
        } else {
            None
        },
    })
}

// Bounded recovery for public type parameters, machine parameter contracts, and machine signatures.

pub(in crate::encoding::recovery) fn type_parameter(
    reader: &mut Reader<'_>,
) -> Result<PackagePolicyTypeParameter, Error> {
    let kind = super::tag!(reader => {
        0 => PackagePolicyTypeParameterKind::Type,
        5 => PackagePolicyTypeParameterKind::Implementation,
        1 => PackagePolicyTypeParameterKind::Const(type_identity(reader)?),
        4 => PackagePolicyTypeParameterKind::Value(type_identity(reader)?),
        2 => PackagePolicyTypeParameterKind::Machine(machine_contract(reader)?),
        3 => PackagePolicyTypeParameterKind::Proposition(
            PackageReviewPropositionParameterSignature {
                parameters: reader.sequence(8, |reader| {
                    Ok(PackageReviewPropositionParameterValue {
                        type_identity: type_identity(reader)?,
                        is_const: reader.boolean()?,
                        is_mutable: reader.boolean()?,
                    })
                })?,
            },
        ),
    });
    Ok(PackagePolicyTypeParameter {
        kind,
        bounds: data_properties(reader)?,
    })
}

pub(in crate::encoding::recovery) fn machine_contract(
    reader: &mut Reader<'_>,
) -> Result<PackagePolicyMachineParameterContract, Error> {
    reader.nested(|reader| {
        Ok(super::tag!(reader => {
            0 => PackagePolicyMachineParameterContract::Structural(machine_signature(reader)?),
            1 => PackagePolicyMachineParameterContract::Nominal {
                trait_identity: nominal(reader)?,
                requirement_identity: nominal(reader)?,
                trait_arguments: reader.sequence(8, type_identity)?,
            },
            2 => PackagePolicyMachineParameterContract::RequirementIdentity,
        }))
    })
}

pub(in crate::encoding::recovery) fn machine_signature(
    reader: &mut Reader<'_>,
) -> Result<PackagePolicyMachineParameterSignature, Error> {
    Ok(PackagePolicyMachineParameterSignature {
        lifetime_parameter_count: reader.usize()?,
        type_parameters: reader.sequence(3, type_parameter)?,
        conformance_bounds: reader.sequence(62, super::signatures::conformance_bound)?,
        parameters: reader.sequence(18, signature_parameter)?,
        return_type: reader.option(type_identity)?,
        contracts: reader.sequence(4, callable_contract)?,
        published_crash: reader.sequence(9, crash_route)?,
        service_reach: reader.sequence(41, service)?,
        service_reach_is_installation_bound: reader.boolean()?,
        synchronous_invocations: reader.sequence(5, synchronous_invocation)?,
        suspends: reader.boolean()?,
        blocks: reader.boolean()?,
        termination: termination(reader)?,
    })
}

// Bounded recovery for public trait shapes, trait requirements, conformance shapes, and operator shapes.

pub(in crate::encoding::recovery) fn trait_shape(
    reader: &mut Reader<'_>,
    schema_version: u16,
) -> Result<PackagePolicyTraitShape, Error> {
    Ok(PackagePolicyTraitShape {
        identity: nominal(reader)?,
        is_boundary: reader.boolean()?,
        lifetime_parameter_count: reader.usize()?,
        type_parameters: reader.sequence(3, type_parameter)?,
        conformance_bounds: reader.sequence(62, conformance_bound)?,
        parents: reader.sequence(58, trait_parent)?,
        requirements: reader.sequence(112, |reader| trait_requirement(reader, schema_version))?,
    })
}

pub(in crate::encoding::recovery) fn trait_requirement(
    reader: &mut Reader<'_>,
    schema_version: u16,
) -> Result<PackagePolicyTraitRequirement, Error> {
    Ok(PackagePolicyTraitRequirement {
        identity: nominal(reader)?,
        spelling: reader.option(operator_spelling)?,
        has_default_realization: reader.boolean()?,
        lifetime_parameter_count: reader.usize()?,
        type_parameters: reader.sequence(3, type_parameter)?,
        conformance_bounds: reader.sequence(62, conformance_bound)?,
        parameters: reader.sequence(18, signature_parameter)?,
        return_type: reader.option(type_identity)?,
        structural_parameter_positions: if schema_version
            == crate::encoding::BASELINE_WITHOUT_STRUCTURAL_PARAMETERS
        {
            None
        } else {
            Some(reader.sequence(4, |reader| reader.u32())?)
        },
        contracts: reader.sequence(4, callable_contract)?,
        published_crash: reader.option(|reader| reader.sequence(9, crash_route))?,
        service_reach: reader.option(|reader| reader.sequence(41, service))?,
        service_reach_is_installation_bound: reader.boolean()?,
        synchronous_invocations: reader.sequence(5, synchronous_invocation)?,
        suspends: reader.boolean()?,
        blocks: reader.boolean()?,
        termination: termination(reader)?,
        admitted_same_stack_bytes: reader.u64()?,
        returned_borrow_origins: reader.sequence(5, returned_borrow_origin)?,
        parameter_qualification_preconditions: reader.sequence(5, parameter_qualification)?,
    })
}

pub(in crate::encoding::recovery::policy::public_api) fn returned_borrow_origin(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewReturnedBorrowOrigin, Error> {
    Ok(PackageReviewReturnedBorrowOrigin {
        origin: super::tag!(reader => {
            1 => PackageReviewBorrowOrigin::Parameter { index: reader.u32()? },
            2 => PackageReviewBorrowOrigin::Invocation { carrier: type_identity(reader)? },
        }),
    })
}

pub(in crate::encoding::recovery::policy::public_api) fn parameter_qualification(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewParameterQualification, Error> {
    Ok(PackageReviewParameterQualification {
        structural_index: reader.u32()?,
        domain: nominal(reader)?,
    })
}

pub(in crate::encoding::recovery) fn conformance_shape(
    reader: &mut Reader<'_>,
) -> Result<PackagePolicyConformanceShape, Error> {
    Ok(PackagePolicyConformanceShape {
        identity: nominal(reader)?,
        lifetime_parameter_count: reader.usize()?,
        type_parameters: reader.sequence(3, type_parameter)?,
        interface: evidence_interface(reader)?,
    })
}

pub(in crate::encoding::recovery) fn operator_shape(
    reader: &mut Reader<'_>,
) -> Result<PackagePolicyOperatorShape, Error> {
    Ok(PackagePolicyOperatorShape {
        coordinate: operator_coordinate(reader)?,
        is_boundary: reader.boolean()?,
        spelling: reader.option(operator_spelling)?,
        lifetime_parameter_count: reader.usize()?,
        type_parameters: reader.sequence(3, type_parameter)?,
        parameters: reader.sequence(18, signature_parameter)?,
        return_type: reader.option(type_identity)?,
        contracts: reader.sequence(4, callable_contract)?,
        published_crash: reader.sequence(9, crash_route)?,
    })
}

// Bounded recovery for public data declaration shapes, supply modes, invariants, and members.

pub(in crate::encoding::recovery) fn data_shape(
    reader: &mut Reader<'_>,
) -> Result<PackagePolicyDataShape, Error> {
    Ok(PackagePolicyDataShape {
        identity: nominal(reader)?,
        kind: super::tag!(reader => {
            0 => PackageReviewDataKind::Ordinary,
            1 => PackageReviewDataKind::Quotient {
                carrier: type_identity(reader)?,
                relation: nominal(reader)?,
            },
        }),
        supply: super::tag!(reader => {
            0 => language_core::DataSupplyMode::CheckedShape,
            1 => language_core::DataSupplyMode::BoundaryOpaque,
        }),
        lifetime_parameter_count: reader.usize()?,
        type_parameters: reader.sequence(3, type_parameter)?,
        properties: data_properties(reader)?,
        zero_gated: reader.boolean()?,
        invariants: reader.sequence(2, contract_fact)?,
        retired_identities: reader.sequence(8, |reader| reader.u64())?,
        members: reader.sequence(19, data_member)?,
    })
}

// Bounded recovery for public domain declaration shapes, predicates, semantic roles, and establishment routes.

pub(in crate::encoding::recovery) fn domain_shape(
    reader: &mut Reader<'_>,
) -> Result<PackagePolicyDomainShape, Error> {
    Ok(PackagePolicyDomainShape {
        identity: nominal(reader)?,
        type_parameters: reader.sequence(3, type_parameter)?,
        target_type: type_identity(reader)?,
        index_arguments: reader.sequence(8, type_identity)?,
        predicate_presence: super::tag!(reader => {
            0 => language_core::DomainPredicatePresence::Absent,
            1 => language_core::DomainPredicatePresence::Present,
        }),
        predicate_facts: reader.sequence(2, contract_fact)?,
        alias_expansion: reader.option(|reader| reader.sequence(2, alias_atom))?,
        classification: super::tag!(reader => {
            0 => None,
            1 => Some(PackageReviewDomainClassification::ProgressProfile),
        }),
        semantic_roles: reader.sequence(1, |reader| {
            Ok(super::tag!(reader => {
                0 => PackageReviewDomainSemanticRole::DenotationDimension,
                1 => PackageReviewDomainSemanticRole::ArithmeticPolicy,
            }))
        })?,
        issuance: reader.option(|reader| reader.sequence(42, establishment_route))?,
    })
}
