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
    let abi = target
        .mixed_structural_scalar_abi
        .as_ref()
        .ok_or(invalid.clone())?;
    let AbstractFunctionResult::Scalar(result) = &abstracted.result else {
        return Err(invalid);
    };
    let parameters = abstracted
        .structural_parameters
        .iter()
        .zip(&abi.structural_parameters)
        .map(|(semantic, target)| crate::structural_unit_input::Parameter { semantic, target })
        .collect::<Vec<_>>();
    if target.machine != abstracted.machine
        || target.machine != optimized.machine
        || target.attachment.is_some()
        || abstracted.attachment.is_some()
        || optimized.attachment.is_some()
        || target.scalar_abi.is_some()
        || !abstracted.parameters.is_empty()
        || !optimized.parameters.is_empty()
        || !abi.scalar_parameters.is_empty()
        || abi.structural_parameters.len() != 1
        || abstracted.structural_parameters.len() != 1
        || optimized.structural_parameters != abstracted.structural_parameters
        || optimized.result != abstracted.result
        || result.scalar_type != ScalarType::Integer(u64_type())
        || abi.result.value != result.value
        || abi.result.scalar_type != result.scalar_type
        || Some(&abi.result.placement) != abi.call_plan.result.as_ref()
        || abi.call_plan.policy != CallingPolicy::native_for_target(native)
        || !abstracted.entry_claims.is_empty()
        || !optimized.entry_claims.is_empty()
        || !optimized.entry_claim_declarations.is_empty()
        || !optimized.content_entry_claims.is_empty()
        || !abstracted.published_service_ceiling.is_empty()
        || !optimized.published_service_ceiling.is_empty()
        || optimized.declared_places
            != abstracted
                .structural_parameters
                .iter()
                .map(|parameter| parameter.place)
                .collect()
        || !crate::structural_unit_input::accepts_borrowed_view(
            &abi.call_plan,
            &parameters,
            &plan.structural_types,
        )
    {
        return Err(invalid);
    }
    let parameter = &abstracted.structural_parameters[0];
    if optimized.structural_places.len() != 1
        || optimized.structural_places[0].id != parameter.place
        || optimized.structural_places[0].kind
            != (semantic_vocabulary::StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            })
    {
        return Err(invalid);
    }
    Ok(abi.call_plan.clone())
}
