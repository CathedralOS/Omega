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
    encoder.field("identity", |encoder| {
        encode_nominal(encoder, &shape.identity)
    })?;
    encoder.field("type_parameters", |encoder| {
        encoder.sequence(&shape.type_parameters, encode_type_parameter)
    })?;
    encoder.field("target_type", |encoder| {
        encode_type_identity(encoder, &shape.target_type)
    })?;
    encoder.field("index_arguments", |encoder| {
        encoder.sequence(&shape.index_arguments, encode_type_identity)
    })?;
    encoder.field("predicate_body", |encoder| {
        match shape.predicate_body {
            language_semantics::DomainPredicateBody::Bodyless => encoder.tag("bodyless", 0),
            language_semantics::DomainPredicateBody::Present => encoder.tag("present", 1),
        };
        Ok(())
    })?;
    encoder.field("predicate_facts", |encoder| {
        encoder.sequence(&shape.predicate_facts, encode_contract_fact)
    })?;
    encoder.field("alias_expansion", |encoder| {
        match &shape.alias_expansion {
            None => encoder.tag("none", 0),
            Some(atoms) => {
                encoder.tag("some", 1);
                encoder.field("atoms", |encoder| {
                    encoder.sequence(atoms, encode_domain_alias_atom)
                })?;
            }
        };
        Ok(())
    })?;
    encoder.field("classification", |encoder| {
        match shape.classification {
            None => encoder.tag("none", 0),
            Some(PackageReviewDomainClassification::ProgressProfile) => {
                encoder.tag("progress_profile", 1)
            }
        };
        Ok(())
    })?;
    encoder.field("semantic_roles", |encoder| {
        encoder.sequence(&shape.semantic_roles, |encoder, role| {
            encoder.field("role", |encoder| {
                match role {
                    PackageReviewDomainSemanticRole::DenotationDimension => {
                        encoder.tag("denotation_dimension", 0)
                    }
                    PackageReviewDomainSemanticRole::ArithmeticPolicy => {
                        encoder.tag("arithmetic_policy", 1)
                    }
                };
                Ok(())
            })?;
            Ok(())
        })
    })?;
    encoder.field("establishment_routes", |encoder| {
        encoder.sequence(
            &shape.establishment_routes,
            encode_domain_establishment_route,
        )
    })
}

pub(crate) fn encode_domain_alias_atom(
    encoder: &mut Encoder,
    atom: &PackageReviewDomainAliasAtom,
) -> Result<(), PackageReviewEncodingError> {
    match atom {
        PackageReviewDomainAliasAtom::Declared(identity) => {
            encoder.tag("declared", 0);
            encoder.field("identity", |encoder| encode_nominal(encoder, identity))
        }
        PackageReviewDomainAliasAtom::Carry(permission) => {
            encoder.tag("carry", 1);
            encoder.field("permission", |encoder| {
                match permission {
                    language_semantics::CarryPermission::AcrossSuspend => {
                        encoder.tag("across_suspend", 0)
                    }
                    language_semantics::CarryPermission::AnyCpu => encoder.tag("any_cpu", 1),
                    language_semantics::CarryPermission::AnyThread => encoder.tag("any_thread", 2),
                    language_semantics::CarryPermission::MovableAddress => {
                        encoder.tag("movable_address", 3)
                    }
                };
                Ok(())
            })?;
            Ok(())
        }
    }
}

pub(crate) fn encode_domain_establishment_route(
    encoder: &mut Encoder,
    route: &PackageReviewDomainEstablishmentRoute,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("kind", |encoder| {
        match route.kind {
            PackageReviewDomainEstablishmentKind::CheckedRequirement => {
                encoder.tag("checked_requirement", 0)
            }
            PackageReviewDomainEstablishmentKind::BoundaryRequirement => {
                encoder.tag("boundary_requirement", 1)
            }
        };
        Ok(())
    })?;
    encoder.field("trait_identity", |encoder| {
        encode_nominal(encoder, &route.trait_identity)
    })?;
    encoder.field("requirement_identity", |encoder| {
        encode_nominal(encoder, &route.requirement_identity)
    })
}
