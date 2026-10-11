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
    let Some(abi) = &target.scalar_abi else {
        return Err(LegalizationError::custody());
    };
    let result_matches = match (abstracted.result.scalar(), abi.result.as_ref()) {
        (None, None) => expected.result.is_none(),
        (Some(declared), Some(actual)) => {
            actual.value == declared.value
                && actual.scalar_type == declared.scalar_type
                && Some(&actual.placement) == expected.result.as_ref()
        }
        _ => false,
    };
    // An ABI's presence describes parameter transport, not its result family.
    // The same declaration/graph join applies to procedures and value returns.
    if !result_matches
        || abi.call_plan != expected
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
        || !target.graph.parameters.is_empty()
        || target.graph.call_plan != expected
        || target.graph.scalar_parameters != abi.parameters
    {
        return Err(LegalizationError::custody());
    }
    Ok(expected)
}
