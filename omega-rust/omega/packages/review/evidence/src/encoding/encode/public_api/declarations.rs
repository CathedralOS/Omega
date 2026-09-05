use super::super::{
    callable_policy::{crash_route, termination},
    declarations::{
        encode_conformance_bound, encode_data_member, encode_data_properties,
        encode_domain_alias_atom, encode_domain_establishment_route, encode_trait_parent,
        encode_type_identity,
    },
    values::{
        contracts::{encode_callable_contract, encode_contract_fact},
        declarations::{
            encode_evidence_interface, encode_operator_coordinate, operator_spelling_tag,
        },
        effects::encode_synchronous_invocation,
        identity::encode_nominal,
    },
};
use super::signatures::formal;
use super::*;

pub(super) fn trait_shape(
    encoder: &mut Encoder,
    shape: &PackagePolicyTraitShape,
) -> Result<(), PackageReviewEncodingError> {
    encode_nominal(encoder, &shape.identity)?;
    encoder.boolean(shape.is_boundary);
    encoder.usize(shape.lifetime_parameter_count)?;
    encoder.sequence(&shape.type_parameters, type_parameter)?;
    encoder.sequence(&shape.conformance_bounds, encode_conformance_bound)?;
    encoder.sequence(&shape.parents, encode_trait_parent)?;
    encoder.sequence(&shape.requirements, |encoder, requirement| {
        encode_nominal(encoder, &requirement.identity)?;
        encoder.option(requirement.spelling.as_ref(), spelling)?;
        encoder.boolean(requirement.has_default_realization);
        encoder.usize(requirement.lifetime_parameter_count)?;
        encoder.sequence(&requirement.type_parameters, type_parameter)?;
        encoder.sequence(&requirement.parameters, |encoder, parameter| {
            formal(
                encoder,
                &parameter.name,
                &parameter.type_identity,
                parameter.is_const,
                parameter.is_mutable,
                parameter.is_self,
            )
        })?;
        encoder.option(requirement.return_type.as_ref(), encode_type_identity)?;
        encoder.sequence(&requirement.contracts, encode_callable_contract)?;
        encoder.sequence(&requirement.published_crash, crash_route)?;
        encoder.sequence(&requirement.service_reach, encode_nominal)?;
        encoder.boolean(requirement.service_reach_is_installation_bound);
        encoder.sequence(
            &requirement.synchronous_invocations,
            encode_synchronous_invocation,
        )?;
        encoder.boolean(requirement.suspends);
        encoder.boolean(requirement.blocks);
        termination(encoder, &requirement.termination)
    })
}

pub(in crate::encoding) fn conformance_shape(
    encoder: &mut Encoder,
    shape: &PackagePolicyConformanceShape,
) -> Result<(), PackageReviewEncodingError> {
    encode_nominal(encoder, &shape.identity)?;
    encoder.usize(shape.lifetime_parameter_count)?;
    encoder.sequence(&shape.type_parameters, type_parameter)?;
    match &shape.subject {
        PackageReviewConformanceSubject::Subjectless => encoder.byte(0),
        PackageReviewConformanceSubject::TypeParameter(ordinal) => {
            encoder.byte(1);
            encoder.u32(*ordinal);
        }
        PackageReviewConformanceSubject::Nominal(identity) => {
            encoder.byte(2);
            encode_nominal(encoder, identity)?;
        }
    }
    encode_evidence_interface(encoder, &shape.interface)
}

pub(super) fn domain_shape(
    encoder: &mut Encoder,
    shape: &PackagePolicyDomainShape,
) -> Result<(), PackageReviewEncodingError> {
    encode_nominal(encoder, &shape.identity)?;
    encoder.sequence(&shape.type_parameters, type_parameter)?;
    encode_type_identity(encoder, &shape.target_type)?;
    encoder.sequence(&shape.index_arguments, encode_type_identity)?;
    encoder.byte(match shape.predicate_body {
        psi_language_semantics::DomainPredicateBody::Bodyless => 0,
        psi_language_semantics::DomainPredicateBody::Present => 1,
    });
    encoder.sequence(&shape.predicate_facts, encode_contract_fact)?;
    encoder.option(shape.alias_expansion.as_deref(), |encoder, atoms| {
        encoder.sequence(atoms, encode_domain_alias_atom)
    })?;
    encoder.byte(match shape.classification {
        None => 0,
        Some(PackageReviewDomainClassification::ProgressProfile) => 1,
    });
    encoder.sequence(&shape.semantic_roles, |encoder, role| {
        encoder.byte(match role {
            PackageReviewDomainSemanticRole::DenotationDimension => 0,
            PackageReviewDomainSemanticRole::ArithmeticPolicy => 1,
        });
        Ok(())
    })?;
    encoder.sequence(
        &shape.establishment_routes,
        encode_domain_establishment_route,
    )
}

pub(super) fn operator_shape(
    encoder: &mut Encoder,
    shape: &PackagePolicyOperatorShape,
) -> Result<(), PackageReviewEncodingError> {
    encode_operator_coordinate(encoder, &shape.coordinate)?;
    encoder.boolean(shape.is_boundary);
    encoder.option(shape.spelling.as_ref(), spelling)?;
    encoder.usize(shape.lifetime_parameter_count)?;
    encoder.sequence(&shape.type_parameters, type_parameter)?;
    encoder.sequence(&shape.parameters, |encoder, parameter| {
        formal(
            encoder,
            &parameter.name,
            &parameter.type_identity,
            parameter.is_const,
            parameter.is_mutable,
            parameter.is_self,
        )
    })?;
    encoder.option(shape.return_type.as_ref(), encode_type_identity)?;
    encoder.sequence(&shape.contracts, encode_callable_contract)?;
    encoder.sequence(&shape.published_crash, crash_route)
}

pub(super) fn data_shape(
    encoder: &mut Encoder,
    shape: &PackagePolicyDataShape,
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
    encoder.sequence(&shape.type_parameters, type_parameter)?;
    encode_data_properties(encoder, shape.properties);
    encoder.boolean(shape.zero_gated);
    encoder.sequence(&shape.invariants, encode_contract_fact)?;
    encoder.sequence(&shape.retired_identities, |encoder, identity| {
        encoder.u64(*identity);
        Ok(())
    })?;
    encoder.sequence(&shape.members, encode_data_member)
}

fn spelling(
    encoder: &mut Encoder,
    value: &psi_language_core::OperatorSpelling,
) -> Result<(), PackageReviewEncodingError> {
    encoder.byte(operator_spelling_tag(*value));
    Ok(())
}
