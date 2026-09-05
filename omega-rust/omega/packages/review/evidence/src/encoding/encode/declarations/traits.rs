use super::super::PackageReviewEncodingError;
use super::super::encoder::Encoder;
use super::super::values::contracts::encode_callable_contract;
use super::super::values::crashes::encode_crash_route;
use super::super::values::effects::{encode_synchronous_invocation, encode_termination};
use super::super::values::identity::encode_nominal;
use super::conformances::encode_conformance_bound;
use super::data::{encode_type_identity, encode_type_parameter};
use crate::record::{
    PackageReviewTraitCompositionKind, PackageReviewTraitParent, PackageReviewTraitRequirement,
    PackageReviewTraitShape,
};

pub(crate) fn encode_trait_shape(
    encoder: &mut Encoder,
    shape: &PackageReviewTraitShape,
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
        encoder.sequence(&shape.type_parameters, encode_type_parameter)
    })?;
    encoder.field("conformance_bounds", |encoder| {
        encoder.sequence(&shape.conformance_bounds, encode_conformance_bound)
    })?;
    encoder.field("parents", |encoder| {
        encoder.sequence(&shape.parents, encode_trait_parent)
    })?;
    encoder.field("requirements", |encoder| {
        encoder.sequence(&shape.requirements, encode_trait_requirement)
    })
}

pub(crate) fn encode_trait_parent(
    encoder: &mut Encoder,
    parent: &PackageReviewTraitParent,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("kind", |encoder| {
        match parent.kind {
            PackageReviewTraitCompositionKind::Policy => encoder.tag("policy", 0),
            PackageReviewTraitCompositionKind::ServiceReach => encoder.tag("service_reach", 1),
        };
        Ok(())
    })?;
    encoder.field("identity", |encoder| {
        encode_nominal(encoder, &parent.identity)
    })?;
    encoder.field("lifetime_arguments", |encoder| {
        encoder.sequence(&parent.lifetime_arguments, |encoder, argument| {
            encoder.field("argument", |encoder| {
                encoder.u32(*argument);
                Ok(())
            })?;
            Ok(())
        })
    })?;
    encoder.field("arguments", |encoder| {
        encoder.sequence(&parent.arguments, encode_type_identity)
    })
}

pub(crate) fn encode_trait_requirement(
    encoder: &mut Encoder,
    requirement: &PackageReviewTraitRequirement,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("identity", |encoder| {
        encode_nominal(encoder, &requirement.identity)
    })?;
    encoder.field("spelling", |encoder| {
        match requirement.spelling {
            None => encoder.tag("none", 0),
            Some(spelling) => {
                encoder.tag("some", 1);
                encoder.field("spelling", |encoder| {
                    match spelling {
                        psi_language_core::OperatorSpelling::Add => encoder.tag("add", 0),
                        psi_language_core::OperatorSpelling::Subtract => encoder.tag("subtract", 1),
                        psi_language_core::OperatorSpelling::Multiply => encoder.tag("multiply", 2),
                        psi_language_core::OperatorSpelling::Divide => encoder.tag("divide", 3),
                        psi_language_core::OperatorSpelling::Modulo => encoder.tag("modulo", 4),
                        psi_language_core::OperatorSpelling::Equal => encoder.tag("equal", 5),
                        psi_language_core::OperatorSpelling::NotEqual => {
                            encoder.tag("not_equal", 6)
                        }
                        psi_language_core::OperatorSpelling::Less => encoder.tag("less", 7),
                        psi_language_core::OperatorSpelling::LessEqual => {
                            encoder.tag("less_equal", 8)
                        }
                        psi_language_core::OperatorSpelling::Greater => encoder.tag("greater", 9),
                        psi_language_core::OperatorSpelling::GreaterEqual => {
                            encoder.tag("greater_equal", 10)
                        }
                        psi_language_core::OperatorSpelling::Index => encoder.tag("index", 11),
                        psi_language_core::OperatorSpelling::Range => encoder.tag("range", 12),
                    };
                    Ok(())
                })?;
            }
        };
        Ok(())
    })?;
    encoder.field("has_default_realization", |encoder| {
        encoder.boolean(requirement.has_default_realization);
        Ok(())
    })?;
    encoder.field("lifetime_parameter_count", |encoder| {
        encoder.usize(requirement.lifetime_parameter_count)
    })?;
    encoder.field("type_parameters", |encoder| {
        encoder.sequence(&requirement.type_parameters, encode_type_parameter)
    })?;
    encoder.field("parameters", |encoder| {
        encoder.sequence(&requirement.parameters, |encoder, parameter| {
            encoder.field("name", |encoder| encoder.string(&parameter.name))?;
            encoder.field("type_identity", |encoder| {
                encode_type_identity(encoder, &parameter.type_identity)
            })?;
            encoder.field("is_const", |encoder| {
                encoder.boolean(parameter.is_const);
                Ok(())
            })?;
            encoder.field("is_mutable", |encoder| {
                encoder.boolean(parameter.is_mutable);
                Ok(())
            })?;
            encoder.field("is_self", |encoder| {
                encoder.boolean(parameter.is_self);
                Ok(())
            })?;
            Ok(())
        })
    })?;
    encoder.field("return_type", |encoder| {
        encode_type_identity(encoder, &requirement.return_type)
    })?;
    encoder.field("contracts", |encoder| {
        encoder.sequence(&requirement.contracts, encode_callable_contract)
    })?;
    encoder.field("published_crash", |encoder| {
        encoder.sequence(&requirement.published_crash, encode_crash_route)
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
        encode_termination(encoder, &requirement.termination)
    })?;
    Ok(())
}
