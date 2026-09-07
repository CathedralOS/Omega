use super::*;
pub(super) fn function_abi(
    native: ::target::NativeTarget,
    target: &TargetFunction,
    abstracted: &AbstractFunction,
    optimized: &PsiOptimizationFunction,
) -> Result<CallPlan, LegalizationError> {
    let invalid = LegalizationError::SourceCustodyMismatch;
    if target.machine != abstracted.machine
        || target.machine != optimized.machine
        || target.attachment != abstracted.attachment
        || target.attachment != optimized.attachment
        || target.mixed_structural_scalar_abi.is_some()
        || optimized.result != abstracted.result
        || !abstracted.structural_parameters.is_empty()
        || !optimized.structural_parameters.is_empty()
        || !optimized.structural_places.is_empty()
        || !abstracted.entry_claims.is_empty()
        || !optimized.entry_claim_declarations.is_empty()
        || !optimized.content_entry_claims.is_empty()
        || !optimized.entry_claims.is_empty()
        || !optimized.declared_places.is_empty()
        || !abstracted.published_service_ceiling.is_empty()
        || !optimized.published_service_ceiling.is_empty()
        || abstracted.parameters.len() != optimized.parameters.len()
        || abstracted
            .parameters
            .iter()
            .zip(&optimized.parameters)
            .enumerate()
            .any(|(index, (declared, actual))| {
                integer_type(declared.scalar_type).is_none()
                    || actual.value != declared.value
                    || actual.scalar_type != declared.scalar_type
                    || actual.site != ValueDefinitionSite::FunctionParameter(index as u32)
            })
    {
        return Err(invalid);
    }
    let result = match &abstracted.result {
        AbstractFunctionResult::Unit => None,
        AbstractFunctionResult::Scalar(result)
            if integer_type(result.scalar_type).is_some() && target.attachment.is_none() =>
        {
            Some(ValueShape::integer(8, 8))
        }
        _ => return Err(invalid),
    };
    let expected = evaluate_call_plan(
        CallingPolicy::native_for_target(native),
        &CallSignature {
            parameters: vec![ValueShape::integer(8, 8); abstracted.parameters.len()],
            result,
        },
    )
    .map_err(|_| invalid.clone())?;
    if expected
        .result
        .as_ref()
        .is_some_and(|value| !register(value))
    {
        return Err(invalid);
    }
    match &abstracted.result {
        AbstractFunctionResult::Scalar(result) => {
            let abi = target
                .fixed_integer_scalar_abi
                .as_ref()
                .ok_or(invalid.clone())?;
            if abi.call_plan != expected
                || abi.result.value != result.value
                || ScalarType::Integer(abi.result.scalar_type) != result.scalar_type
                || Some(&abi.result.placement) != expected.result.as_ref()
                || abi.parameters.len() != abstracted.parameters.len()
                || abi
                    .parameters
                    .iter()
                    .zip(&abstracted.parameters)
                    .zip(&expected.parameters)
                    .any(|((actual, declared), placement)| {
                        actual.value != declared.value
                            || ScalarType::Integer(actual.scalar_type) != declared.scalar_type
                            || actual.placement != *placement
                    })
            {
                return Err(invalid);
            }
        }
        AbstractFunctionResult::Unit => {
            let TargetOperation::UnitBody(body) = &target.operation else {
                return Err(invalid);
            };
            if target.fixed_integer_scalar_abi.is_some()
                || !body.parameters.is_empty()
                || body.call_plan != expected
                || body.scalar_parameters.len() != abstracted.parameters.len()
                || body
                    .scalar_parameters
                    .iter()
                    .zip(&abstracted.parameters)
                    .zip(&expected.parameters)
                    .any(|((actual, declared), placement)| {
                        actual.value != declared.value
                            || actual.scalar_type != declared.scalar_type
                            || actual.placement != *placement
                    })
            {
                return Err(invalid);
            }
        }
        _ => return Err(invalid),
    }
    Ok(expected)
}
