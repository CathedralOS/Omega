use super::{
    AbstractFunction, AbstractFunctionResult, CallPlan, CallSignature, CallingPolicy,
    PsiOptimizationFunction, TargetFunction, ValueDefinitionSite, evaluate_call_plan, scalar_shape,
};
use crate::LegalizationError;
use crate::legalization::scalar_graph_input::scalar_register;
pub(super) fn function_abi(
    native: ::target::NativeTarget,
    target: &TargetFunction,
    abstracted: &AbstractFunction,
    optimized: &PsiOptimizationFunction,
) -> Result<CallPlan, LegalizationError> {
    if target.machine != abstracted.machine
        || target.machine != optimized.machine
        || (target.attachment != abstracted.attachment
            || target.nominal_cleanup_subject != abstracted.nominal_cleanup_subject)
        || (target.attachment != optimized.attachment
            || target.nominal_cleanup_subject != optimized.nominal_cleanup_subject)
        || target.mixed_structural_scalar_abi.is_some()
        || optimized.result != abstracted.result
        || !abstracted.structural_parameters.is_empty()
        || !optimized.structural_parameters.is_empty()
        || !super::place_declarations::matches_producers(optimized)
        || !abstracted.entry_claims.is_empty()
        || !optimized.entry_claim_declarations.is_empty()
        || !optimized.content_entry_claims.is_empty()
        || !optimized.entry_claims.is_empty()
        || abstracted.published_service_ceiling != optimized.published_service_ceiling
        || abstracted.parameters.len() != optimized.parameters.len()
        || abstracted
            .parameters
            .iter()
            .zip(&optimized.parameters)
            .enumerate()
            .any(|(index, (declared, actual))| {
                scalar_shape(declared.scalar_type).is_none()
                    || actual.value != declared.value
                    || actual.scalar_type != declared.scalar_type
                    || actual.site != ValueDefinitionSite::FunctionParameter(index as u32)
            })
    {
        return Err(LegalizationError::custody());
    }
    let result = match &abstracted.result {
        AbstractFunctionResult::Unit => None,
        // An attachment names the nominal specialization, not an implicit
        // receiver argument. Exact attachment equality is checked above;
        // runtime receiver storage still requires structural parameter custody.
        AbstractFunctionResult::Scalar(result) if scalar_shape(result.scalar_type).is_some() => {
            scalar_shape(result.scalar_type)
        }
        _ => {
            return Err(LegalizationError::custody());
        }
    };
    let expected_parameters = abstracted
        .parameters
        .iter()
        .map(|parameter| scalar_shape(parameter.scalar_type).ok_or(LegalizationError::custody()))
        .collect::<Result<Vec<_>, _>>()?;
    let expected = evaluate_call_plan(
        CallingPolicy::native_for_target(native),
        &CallSignature {
            parameters: expected_parameters,
            result,
        },
    )
    .map_err(|_| LegalizationError::custody())?;
    if expected
        .result
        .as_ref()
        .is_some_and(|value| !scalar_register(value))
    {
        return Err(LegalizationError::custody());
    }
    match &abstracted.result {
        AbstractFunctionResult::Scalar(result) => {
            let Some(abi) = &target.scalar_abi else {
                return Err(LegalizationError::custody());
            };
            if abi.call_plan != expected
                || abi.result.value != result.value
                || abi.result.scalar_type != result.scalar_type
                || Some(&abi.result.placement) != expected.result.as_ref()
                || abi.parameters.len() != abstracted.parameters.len()
                || abi
                    .parameters
                    .iter()
                    .zip(&abstracted.parameters)
                    .zip(&expected.parameters)
                    .any(|((actual, declared), placement)| {
                        actual.value != declared.value
                            || actual.scalar_type != declared.scalar_type
                            || actual.placement != *placement
                    })
            {
                return Err(LegalizationError::custody());
            }
        }
        AbstractFunctionResult::Unit => {
            let graph = &target.graph;
            let (parameters, scalar_parameters, call_plan) = (
                &graph.parameters,
                &graph.scalar_parameters,
                &graph.call_plan,
            );
            if target.scalar_abi.is_some()
                || !parameters.is_empty()
                || *call_plan != expected
                || scalar_parameters.len() != abstracted.parameters.len()
                || scalar_parameters
                    .iter()
                    .zip(&abstracted.parameters)
                    .zip(&expected.parameters)
                    .any(|((actual, declared), placement)| {
                        actual.value != declared.value
                            || actual.scalar_type != declared.scalar_type
                            || actual.placement != *placement
                    })
            {
                return Err(LegalizationError::custody());
            }
        }
        _ => {
            return Err(LegalizationError::custody());
        }
    }
    Ok(expected)
}
