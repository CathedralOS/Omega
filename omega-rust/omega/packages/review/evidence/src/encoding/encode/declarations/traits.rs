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
    encode_nominal(encoder, &shape.identity)?;
    encoder.boolean(shape.is_boundary);
    encoder.usize(shape.lifetime_parameter_count)?;
    encoder.sequence(&shape.type_parameters, encode_type_parameter)?;
    encoder.sequence(&shape.conformance_bounds, encode_conformance_bound)?;
    encoder.sequence(&shape.parents, encode_trait_parent)?;
    encoder.sequence(&shape.requirements, encode_trait_requirement)
}

pub(crate) fn encode_trait_parent(
    encoder: &mut Encoder,
    parent: &PackageReviewTraitParent,
) -> Result<(), PackageReviewEncodingError> {
    encoder.byte(match parent.kind {
        PackageReviewTraitCompositionKind::Policy => 0,
        PackageReviewTraitCompositionKind::ServiceReach => 1,
    });
    encode_nominal(encoder, &parent.identity)?;
    encoder.sequence(&parent.lifetime_arguments, |encoder, argument| {
        encoder.u32(*argument);
        Ok(())
    })?;
    encoder.sequence(&parent.arguments, encode_type_identity)
}

pub(crate) fn encode_trait_requirement(
    encoder: &mut Encoder,
    requirement: &PackageReviewTraitRequirement,
) -> Result<(), PackageReviewEncodingError> {
    encode_nominal(encoder, &requirement.identity)?;
    match requirement.spelling {
        None => encoder.byte(0),
        Some(spelling) => {
            encoder.byte(1);
            encoder.byte(match spelling {
                psi_language_core::OperatorSpelling::Add => 0,
                psi_language_core::OperatorSpelling::Subtract => 1,
                psi_language_core::OperatorSpelling::Multiply => 2,
                psi_language_core::OperatorSpelling::Divide => 3,
                psi_language_core::OperatorSpelling::Modulo => 4,
                psi_language_core::OperatorSpelling::Equal => 5,
                psi_language_core::OperatorSpelling::NotEqual => 6,
                psi_language_core::OperatorSpelling::Less => 7,
                psi_language_core::OperatorSpelling::LessEqual => 8,
                psi_language_core::OperatorSpelling::Greater => 9,
                psi_language_core::OperatorSpelling::GreaterEqual => 10,
                psi_language_core::OperatorSpelling::Index => 11,
                psi_language_core::OperatorSpelling::Range => 12,
            });
        }
    }
    encoder.boolean(requirement.has_default_realization);
    encoder.usize(requirement.lifetime_parameter_count)?;
    encoder.sequence(&requirement.type_parameters, encode_type_parameter)?;
    encoder.sequence(&requirement.parameters, |encoder, parameter| {
        encoder.string(&parameter.name)?;
        encode_type_identity(encoder, &parameter.type_identity)?;
        encoder.boolean(parameter.is_const);
        encoder.boolean(parameter.is_mutable);
        encoder.boolean(parameter.is_self);
        Ok(())
    })?;
    encode_type_identity(encoder, &requirement.return_type)?;
    encoder.sequence(&requirement.contracts, encode_callable_contract)?;
    encoder.sequence(&requirement.published_crash, encode_crash_route)?;
    encoder.sequence(&requirement.service_reach, encode_nominal)?;
    encoder.boolean(requirement.service_reach_is_installation_bound);
    encoder.sequence(
        &requirement.synchronous_invocations,
        encode_synchronous_invocation,
    )?;
    encoder.boolean(requirement.suspends);
    encoder.boolean(requirement.blocks);
    encode_termination(encoder, &requirement.termination)?;
    Ok(())
}
