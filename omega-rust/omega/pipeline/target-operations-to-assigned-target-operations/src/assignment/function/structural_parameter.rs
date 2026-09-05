use crate::assignment::placement::{
    validate_direct_structural_return_placement, validate_structural_placement,
};
use crate::assignment::shared::*;

pub(super) fn assign(
    operation: &TargetOperation,
    target: NativeTarget,
) -> Result<AssignedOperation, AssignmentError> {
    let architecture = target.architecture;
    Ok(match operation {
        TargetOperation::ReturnStructuralParameter {
            call_plan,
            scalar_parameters,
            parameters,
            source,
            result,
            shape,
            source_placement,
            result_placement,
            psi_edge,
            returned_claims,
            trivial_affine_locals,
            trivial_affine_discards,
        } => {
            let source_index = scalar_parameters.len();
            if parameters.first() != Some(source) {
                return Err(AssignmentError::UnsupportedStructuralPlacement(
                    source.place,
                ));
            }
            let exact_claimful_linear = scalar_parameters.is_empty()
                && source.multiplicity == terminal_psi::StructuralMultiplicity::Linear
                && result.multiplicity == terminal_psi::StructuralMultiplicity::Linear
                && returned_claims.len() == 1
                && trivial_affine_discards.len() + 1
                    == parameters.len() + trivial_affine_locals.len();
            let exact_claim_free_affine_mixed = scalar_parameters.len() == 1
                && parameters.len() == 1
                && source.multiplicity == terminal_psi::StructuralMultiplicity::Affine
                && result.multiplicity == terminal_psi::StructuralMultiplicity::Affine
                && source.access == terminal_psi::StructuralAccess::Owned
                && source.qualifications.is_empty()
                && source.projected_qualifications.is_empty()
                && result.qualifications.is_empty()
                && result.projected_qualifications.is_empty()
                && returned_claims.is_empty()
                && trivial_affine_locals.is_empty()
                && trivial_affine_discards.is_empty();
            let expected_call_plan = evaluate_call_plan(
                CallingPolicy::native_for_target(target),
                &CallSignature {
                    parameters: call_plan
                        .parameters
                        .iter()
                        .map(|placement| placement.shape)
                        .collect(),
                    result: Some(*shape),
                },
            )
            .map_err(|_| AssignmentError::UnsupportedStructuralPlacement(source.place))?;
            if *call_plan != expected_call_plan
                || call_plan.parameters.len() != scalar_parameters.len() + parameters.len()
                || scalar_parameters
                    .iter()
                    .zip(&call_plan.parameters)
                    .any(|(parameter, placement)| {
                        parameter.placement != *placement
                            || parameter.scalar_type.is_address()
                            || !matches!(parameter.scalar_type.bits(), 8 | 16 | 32 | 64)
                            || placement.shape
                                != calling_conventions::ValueShape::integer(
                                    parameter.scalar_type.bits() / 8,
                                    parameter.scalar_type.bits() / 8,
                                )
                    })
                || call_plan.parameters.get(source_index) != Some(source_placement)
                || call_plan.result.as_ref() != Some(result_placement)
                || source.place == result.place
                || (!exact_claimful_linear && !exact_claim_free_affine_mixed)
                || source.structural_type != result.structural_type
                || source.qualifications != result.qualifications
                || parameters.iter().enumerate().any(|(index, parameter)| {
                    usize::try_from(parameter.position) != Ok(index) || parameter.is_self
                })
                || parameters.iter().skip(1).any(|parameter| {
                    parameter.place == source.place
                        || parameter.place == result.place
                        || !parameter.qualifications.is_empty()
                })
                || parameters
                    .iter()
                    .map(|parameter| parameter.place)
                    .collect::<std::collections::BTreeSet<_>>()
                    .len()
                    != parameters.len()
                || trivial_affine_locals
                    .iter()
                    .enumerate()
                    .any(|(index, (_, local, local_type))| {
                    !matches!(
                        local.kind,
                        semantic_vocabulary::StructuralPlaceKind::TrivialAffineLocal {
                            declaration_ordinal,
                            structural_type,
                            construction: None,
                        } if usize::try_from(declaration_ordinal) == Ok(index)
                            && structural_type == local_type.id
                    ) || local.id == source.place
                        || local.id == result.place
                        || parameters.iter().any(|parameter| parameter.place == local.id)
                        || local_type.identity.is_empty()
                        || !matches!(
                            local_type.shape,
                            terminal_psi::StructuralTypeShape::Record { ref fields } if fields.is_empty()
                        )
                })
                || trivial_affine_locals
                    .iter()
                    .map(|(_, local, _)| local.id)
                    .collect::<std::collections::BTreeSet<_>>()
                    .len()
                    != trivial_affine_locals.len()
                || trivial_affine_discards
                    != &trivial_affine_locals
                        .iter()
                        .rev()
                        .map(|(_, local, _)| local.id)
                        .chain(parameters.iter().skip(1).rev().map(|parameter| parameter.place))
                        .collect::<Vec<_>>()
                || parameters
                    .iter()
                    .skip(1)
                    .any(|parameter| parameter.multiplicity != terminal_psi::StructuralMultiplicity::Affine)
            {
                return Err(AssignmentError::UnsupportedStructuralPlacement(
                    source.place,
                ));
            }
            for (parameter, placement) in parameters
                .iter()
                .zip(call_plan.parameters.iter().skip(scalar_parameters.len()))
            {
                if parameter.place == source.place {
                    validate_direct_structural_return_placement(
                        parameter.place,
                        placement,
                        architecture,
                    )?;
                } else {
                    validate_structural_placement(parameter.place, placement, architecture)?;
                }
            }
            validate_direct_structural_return_placement(
                result.place,
                result_placement,
                architecture,
            )?;
            AssignedOperation::ReturnStructuralParameter {
                call_plan: call_plan.clone(),
                scalar_parameters: scalar_parameters.clone(),
                parameters: parameters.clone(),
                source: source.clone(),
                result: result.clone(),
                shape: *shape,
                source_placement: source_placement.clone(),
                result_placement: result_placement.clone(),
                psi_edge: *psi_edge,
                returned_claims: returned_claims.clone(),
                trivial_affine_locals: trivial_affine_locals.clone(),
                trivial_affine_discards: trivial_affine_discards.clone(),
            }
        }
        _ => unreachable!("structural-parameter assignment receives its exact carrier"),
    })
}
