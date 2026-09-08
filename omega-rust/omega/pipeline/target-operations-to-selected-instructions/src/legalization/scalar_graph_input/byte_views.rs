//! Exact immutable descriptor input for scalar byte observations.
use super::*;

pub(super) fn validate(
    target: &TargetFunction,
    abstracted: &AbstractFunction,
    optimized: &PsiOptimizationFunction,
    native: ::target::NativeTarget,
    plan: &AbstractOperationPlan,
) -> Result<CallPlan, LegalizationError> {
    let invalid = LegalizationError::SourceCustodyMismatch;
    let (call_plan, scalar_parameters, structural_parameters) = match (
        &abstracted.result,
        &target.operation,
        &target.mixed_structural_scalar_abi,
    ) {
        (AbstractFunctionResult::Unit, TargetOperation::UnitBody(body), None)
            if optimized.blocks.len() == 1 && body.call_plan.result.is_none() =>
        {
            (&body.call_plan, &body.scalar_parameters, &body.parameters)
        }
        (AbstractFunctionResult::Scalar(result), _, Some(abi))
            if result.scalar_type == ScalarType::Integer(u64_type())
                && abi.result.value == result.value
                && abi.result.scalar_type == result.scalar_type
                && Some(&abi.result.placement) == abi.call_plan.result.as_ref() =>
        {
            (
                &abi.call_plan,
                &abi.scalar_parameters,
                &abi.structural_parameters,
            )
        }
        _ => return Err(invalid),
    };
    let parameters = abstracted
        .structural_parameters
        .iter()
        .zip(structural_parameters)
        .map(|(semantic, target)| crate::structural_unit_input::Parameter { semantic, target })
        .collect::<Vec<_>>();
    if target.machine != abstracted.machine
        || target.machine != optimized.machine
        || target.attachment.is_some()
        || abstracted.attachment.is_some()
        || optimized.attachment.is_some()
        || target.scalar_abi.is_some()
        || abstracted.parameters.len() != optimized.parameters.len()
        || scalar_parameters.len() != abstracted.parameters.len()
        || call_plan.parameters.len() != abstracted.parameters.len() + 1
        || scalar_parameters
            .iter()
            .zip(&abstracted.parameters)
            .zip(&optimized.parameters)
            .zip(&call_plan.parameters)
            .enumerate()
            .any(|(position, (((actual, declared), optimized), placement))| {
                actual.value != declared.value
                    || ![ScalarType::Integer(u64_type()), ScalarType::Boolean]
                        .contains(&actual.scalar_type)
                    || declared.scalar_type != actual.scalar_type
                    || optimized.value != declared.value
                    || optimized.scalar_type != declared.scalar_type
                    || optimized.site != ValueDefinitionSite::FunctionParameter(position as u32)
                    || scalar_shape(actual.scalar_type) != Some(placement.shape)
                    || actual.placement != *placement
            })
        || structural_parameters.len() != 1
        || abstracted.structural_parameters.len() != 1
        || optimized.structural_parameters != abstracted.structural_parameters
        || optimized.result != abstracted.result
        || call_plan.policy != CallingPolicy::native_for_target(native)
        || !abstracted.entry_claims.is_empty()
        || !optimized.entry_claims.is_empty()
        || !optimized.entry_claim_declarations.is_empty()
        || !optimized.content_entry_claims.is_empty()
        || !abstracted.published_service_ceiling.is_empty()
        || !optimized.published_service_ceiling.is_empty()
        || optimized.declared_places
            != optimized
                .structural_places
                .iter()
                .map(|place| place.id)
                .collect()
        || !crate::structural_unit_input::accepts_borrowed_view(
            call_plan,
            &parameters,
            &plan.structural_types,
        )
    {
        return Err(invalid);
    }
    let parameter = &abstracted.structural_parameters[0];
    let subslices = optimized
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .filter_map(|node| {
            if let AbstractOperation::ByteSequenceSubslice {
                psi_operation,
                result,
                ..
            } = &node.operation
            {
                Some((*psi_operation, result))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    if optimized.structural_places.len() != 1 + subslices.len()
        || optimized.structural_places[0].id != parameter.place
        || optimized.structural_places[0].kind
            != (semantic_vocabulary::StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            })
    {
        return Err(invalid);
    }
    for place in &optimized.structural_places[1..] {
        if !subslices.iter().any(|(operation, result)| {
            place.id == result.place
                && result.structural_type == parameter.structural_type
                && result.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
                && result.qualifications.is_empty()
                && result.projected_qualifications.is_empty()
                && result.claims.is_empty()
                && place.kind
                    == (semantic_vocabulary::StructuralPlaceKind::OperationResult {
                        producer: *operation,
                        structural_type: result.structural_type,
                    })
        }) {
            return Err(invalid);
        }
    }
    Ok(call_plan.clone())
}

// Whole-unit validation checks the exact producer and dominance. This predicate
// restricts the native route to parameters and derived immutable byte views.
pub(super) fn contains_view(
    function: &PsiOptimizationFunction,
    place: semantic_vocabulary::PlaceId,
) -> bool {
    function.structural_parameters.iter().any(|parameter| parameter.place == place)
        || function.blocks.iter().flat_map(|block| &block.nodes).any(|node|
            matches!(&node.operation, AbstractOperation::ByteSequenceSubslice { result, .. } if result.place == place))
}
