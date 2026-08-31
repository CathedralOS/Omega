use super::super::PackageReviewEncodingError;
use super::super::encoder::Encoder;
use super::super::values::contracts::encode_contract_fact;
use super::super::values::identity::encode_nominal;
use super::data::{encode_type_identity, encode_type_parameter};
use crate::record::{
    PackageReviewDomainAliasAtom, PackageReviewDomainClassification,
    PackageReviewDomainEstablishmentKind, PackageReviewDomainEstablishmentRoute,
    PackageReviewDomainSemanticRole, PackageReviewDomainShape,
};

pub(crate) fn encode_domain_shape(
    encoder: &mut Encoder,
    shape: &PackageReviewDomainShape,
) -> Result<(), PackageReviewEncodingError> {
    encode_nominal(encoder, &shape.identity)?;
    encoder.sequence(&shape.type_parameters, encode_type_parameter)?;
    encode_type_identity(encoder, &shape.target_type)?;
    encoder.sequence(&shape.index_arguments, encode_type_identity)?;
    encoder.byte(match shape.predicate_body {
        psi_language_semantics::DomainPredicateBody::Bodyless => 0,
        psi_language_semantics::DomainPredicateBody::Present => 1,
    });
    encoder.sequence(&shape.predicate_facts, encode_contract_fact)?;
    match &shape.alias_expansion {
        None => encoder.byte(0),
        Some(atoms) => {
            encoder.byte(1);
            encoder.sequence(atoms, encode_domain_alias_atom)?;
        }
    }
    match shape.classification {
        None => encoder.byte(0),
        Some(PackageReviewDomainClassification::ProgressProfile) => encoder.byte(1),
    }
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

pub(crate) fn encode_domain_alias_atom(
    encoder: &mut Encoder,
    atom: &PackageReviewDomainAliasAtom,
) -> Result<(), PackageReviewEncodingError> {
    match atom {
        PackageReviewDomainAliasAtom::Declared(identity) => {
            encoder.byte(0);
            encode_nominal(encoder, identity)
        }
        PackageReviewDomainAliasAtom::Carry(permission) => {
            encoder.byte(1);
            encoder.byte(match permission {
                psi_language_semantics::CarryPermission::AcrossSuspend => 0,
                psi_language_semantics::CarryPermission::AnyCpu => 1,
                psi_language_semantics::CarryPermission::AnyThread => 2,
                psi_language_semantics::CarryPermission::MovableAddress => 3,
            });
            Ok(())
        }
    }
}

pub(crate) fn encode_domain_establishment_route(
    encoder: &mut Encoder,
    route: &PackageReviewDomainEstablishmentRoute,
) -> Result<(), PackageReviewEncodingError> {
    encoder.byte(match route.kind {
        PackageReviewDomainEstablishmentKind::CheckedRequirement => 0,
        PackageReviewDomainEstablishmentKind::BoundaryRequirement => 1,
    });
    encode_nominal(encoder, &route.trait_identity)?;
    encode_nominal(encoder, &route.requirement_identity)
}
