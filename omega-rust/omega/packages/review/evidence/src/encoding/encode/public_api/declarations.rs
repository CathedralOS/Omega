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
            encode_evidence_interface, encode_operator_coordinate, operator_spelling_name,
            operator_spelling_tag,
        },
        effects::encode_synchronous_invocation,
        identity::encode_nominal,
    },
};
use super::signatures::formal;
use super::*;

pub(in crate::encoding::encode) fn trait_shape(
    encoder: &mut Encoder,
    shape: &PackagePolicyTraitShape,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("identity", |encoder| {
        encode_nominal(encoder, &shape.identity)
    })?;
    encoder.field("is_boundary", |encoder| {
        encoder.boolean(shape.is_boundary);
        Ok(())
    })?;
    encoder.field("lifetime_parameter_count", |encoder| {
        encoder.usize(shape.lifetime_parameter_count)
    })?;
    encoder.field("type_parameters", |encoder| {
        encoder.sequence(&shape.type_parameters, type_parameter)
    })?;
    encoder.field("conformance_bounds", |encoder| {
        encoder.sequence(&shape.conformance_bounds, encode_conformance_bound)
    })?;
    encoder.field("parents", |encoder| {
        encoder.sequence(&shape.parents, encode_trait_parent)
    })?;
    encoder.field("requirements", |encoder| {
        encoder.sequence(&shape.requirements, |encoder, requirement| {
            encoder.field("identity", |encoder| {
                encode_nominal(encoder, &requirement.identity)
            })?;
            encoder.field("spelling", |encoder| {
                encoder.option(requirement.spelling.as_ref(), spelling)
            })?;
            encoder.field("has_default_realization", |encoder| {
                encoder.boolean(requirement.has_default_realization);
                Ok(())
            })?;
            encoder.field("lifetime_parameter_count", |encoder| {
                encoder.usize(requirement.lifetime_parameter_count)
            })?;
            encoder.field("type_parameters", |encoder| {
                encoder.sequence(&requirement.type_parameters, type_parameter)
            })?;
            encoder.field("parameters", |encoder| {
                encoder.sequence(&requirement.parameters, |encoder, parameter| {
                    formal(
                        encoder,
                        &parameter.name,
                        &parameter.type_identity,
                        parameter.is_const,
                        parameter.is_mutable,
                        parameter.is_self,
                    )
                })
            })?;
            encoder.field("return_type", |encoder| {
                encoder.option(requirement.return_type.as_ref(), encode_type_identity)
            })?;
            encoder.field("contracts", |encoder| {
                encoder.sequence(&requirement.contracts, encode_callable_contract)
            })?;
            encoder.field("published_crash", |encoder| {
                encoder.sequence(&requirement.published_crash, crash_route)
            })?;
            encoder.field("service_reach", |encoder| {
                encoder.sequence(&requirement.service_reach, encode_nominal)
            })?;
            encoder.field("service_reach_is_installation_bound", |encoder| {
                encoder.boolean(requirement.service_reach_is_installation_bound);
                Ok(())
            })?;
            encoder.field("synchronous_invocations", |encoder| {
                encoder.sequence(
                    &requirement.synchronous_invocations,
                    encode_synchronous_invocation,
                )
            })?;
            encoder.field("suspends", |encoder| {
                encoder.boolean(requirement.suspends);
                Ok(())
            })?;
            encoder.field("blocks", |encoder| {
                encoder.boolean(requirement.blocks);
                Ok(())
            })?;
            encoder.field("termination", |encoder| {
                termination(encoder, &requirement.termination)
            })
        })
    })
}

pub(in crate::encoding) fn conformance_shape(
    encoder: &mut Encoder,
    shape: &PackagePolicyConformanceShape,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("identity", |encoder| {
        encode_nominal(encoder, &shape.identity)
    })?;
    encoder.field("lifetime_parameter_count", |encoder| {
        encoder.usize(shape.lifetime_parameter_count)
    })?;
    encoder.field("type_parameters", |encoder| {
        encoder.sequence(&shape.type_parameters, type_parameter)
    })?;
    encoder.field("subject", |encoder| {
        match &shape.subject {
            PackageReviewConformanceSubject::Subjectless => encoder.tag("subjectless", 0),
            PackageReviewConformanceSubject::TypeParameter(ordinal) => {
                encoder.tag("type_parameter", 1);
                encoder.field("value", |encoder| {
                    encoder.u32(*ordinal);
                    Ok(())
                })?;
            }
            PackageReviewConformanceSubject::Nominal(identity) => {
                encoder.tag("nominal", 2);
                encoder.field("identity", |encoder| encode_nominal(encoder, identity))?;
            }
        };
        Ok(())
    })?;
    encoder.field("interface", |encoder| {
        encode_evidence_interface(encoder, &shape.interface)
    })
}

pub(in crate::encoding::encode) fn domain_shape(
    encoder: &mut Encoder,
    shape: &PackagePolicyDomainShape,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("identity", |encoder| {
        encode_nominal(encoder, &shape.identity)
    })?;
    encoder.field("type_parameters", |encoder| {
        encoder.sequence(&shape.type_parameters, type_parameter)
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
        encoder.option(shape.alias_expansion.as_deref(), |encoder, atoms| {
            encoder.field("atoms", |encoder| {
                encoder.sequence(atoms, encode_domain_alias_atom)
            })
        })
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

pub(in crate::encoding::encode) fn operator_shape(
    encoder: &mut Encoder,
    shape: &PackagePolicyOperatorShape,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("coordinate", |encoder| {
        encode_operator_coordinate(encoder, &shape.coordinate)
    })?;
    encoder.field("is_boundary", |encoder| {
        encoder.boolean(shape.is_boundary);
        Ok(())
    })?;
    encoder.field("spelling", |encoder| {
        encoder.option(shape.spelling.as_ref(), spelling)
    })?;
    encoder.field("lifetime_parameter_count", |encoder| {
        encoder.usize(shape.lifetime_parameter_count)
    })?;
    encoder.field("type_parameters", |encoder| {
        encoder.sequence(&shape.type_parameters, type_parameter)
    })?;
    encoder.field("parameters", |encoder| {
        encoder.sequence(&shape.parameters, |encoder, parameter| {
            formal(
                encoder,
                &parameter.name,
                &parameter.type_identity,
                parameter.is_const,
                parameter.is_mutable,
                parameter.is_self,
            )
        })
    })?;
    encoder.field("return_type", |encoder| {
        encoder.option(shape.return_type.as_ref(), encode_type_identity)
    })?;
    encoder.field("contracts", |encoder| {
        encoder.sequence(&shape.contracts, encode_callable_contract)
    })?;
    encoder.field("published_crash", |encoder| {
        encoder.sequence(&shape.published_crash, crash_route)
    })
}

pub(in crate::encoding::encode) fn data_shape(
    encoder: &mut Encoder,
    shape: &PackagePolicyDataShape,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("identity", |encoder| {
        encode_nominal(encoder, &shape.identity)
    })?;
    encoder.field("kind", |encoder| {
        match &shape.kind {
            PackageReviewDataKind::Ordinary => encoder.tag("ordinary", 0),
            PackageReviewDataKind::Quotient { carrier, relation } => {
                encoder.tag("quotient", 1);
                encoder.field("carrier", |encoder| encode_type_identity(encoder, carrier))?;
                encoder.field("relation", |encoder| encode_nominal(encoder, relation))?;
            }
        };
        Ok(())
    })?;
    encoder.field("supply", |encoder| {
        match shape.supply {
            language_semantics::DataSupplyMode::CheckedShape => encoder.tag("checked_shape", 0),
            language_semantics::DataSupplyMode::BoundaryOpaque => encoder.tag("boundary_opaque", 1),
        };
        Ok(())
    })?;
    encoder.field("lifetime_parameter_count", |encoder| {
        encoder.usize(shape.lifetime_parameter_count)
    })?;
    encoder.field("type_parameters", |encoder| {
        encoder.sequence(&shape.type_parameters, type_parameter)
    })?;
    encoder.field("properties", |encoder| {
        encode_data_properties(encoder, shape.properties);
        Ok(())
    })?;
    encoder.field("zero_gated", |encoder| {
        encoder.boolean(shape.zero_gated);
        Ok(())
    })?;
    encoder.field("invariants", |encoder| {
        encoder.sequence(&shape.invariants, encode_contract_fact)
    })?;
    encoder.field("retired_identities", |encoder| {
        encoder.sequence(&shape.retired_identities, |encoder, identity| {
            encoder.field("identity", |encoder| {
                encoder.u64(*identity);
                Ok(())
            })?;
            Ok(())
        })
    })?;
    encoder.field("members", |encoder| {
        encoder.sequence(&shape.members, encode_data_member)
    })
}

fn spelling(
    encoder: &mut Encoder,
    value: &language_core::OperatorSpelling,
) -> Result<(), PackageReviewEncodingError> {
    encoder.tag(
        operator_spelling_name(*value),
        operator_spelling_tag(*value),
    );
    Ok(())
}
