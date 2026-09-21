use super::{
    AbstractFunction, AbstractFunctionResult, AbstractOperation, CallPlan, CallSignature,
    CallingPolicy, PsiOptimizationFunction, TargetFunction, ValueDefinitionSite, ValueShape,
    evaluate_call_plan, scalar_shape,
};
use crate::LegalizationError;
use crate::legalization::scalar_graph_input::scalar_register;
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
        || !(super::primitive_locals::roster(optimized)
            || super::literals::roster(optimized)
            || (abstracted.result == AbstractFunctionResult::Unit
                && super::read_byte::roster(optimized))
            || (abstracted.result == AbstractFunctionResult::Unit
                && super::literals::provider_metadata_roster(optimized)))
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
        return Err(invalid);
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
            return Err(invalid);
        }
    };
    // Borrowed descriptor parameters are zero-code leading declarations; each
    // occupies a trailing `{instance, table}` pointer pair whose dense lane
    // ordinal and borrowed access the declaration row must retain exactly.
    let declared_dynamic_parameters = abstracted
        .operations
        .iter()
        .take_while(|operation| {
            matches!(
                operation,
                AbstractOperation::DynamicDescriptorParameter { .. }
            )
        })
        .filter_map(|operation| match operation {
            AbstractOperation::DynamicDescriptorParameter { parameter } => Some(parameter),
            _ => None,
        })
        .collect::<Vec<_>>();
    if declared_dynamic_parameters
        .iter()
        .enumerate()
        .any(|(index, parameter)| {
            parameter.owner != abstracted.machine
                || parameter.ordinal != index as u32
                || parameter.source_position != (abstracted.parameters.len() + index) as u32
                || !matches!(
                    parameter.access,
                    terminal_psi::StructuralAccess::SharedBorrow
                        | terminal_psi::StructuralAccess::MutableBorrow
                )
        })
    {
        return Err(invalid);
    }
    let pointer_size = u16::try_from(native.pointer_size).map_err(|_| invalid.clone())?;
    let pointer_alignment = u16::try_from(native.pointer_alignment).map_err(|_| invalid.clone())?;
    let mut expected_parameters = abstracted
        .parameters
        .iter()
        .map(|parameter| scalar_shape(parameter.scalar_type).ok_or(invalid.clone()))
        .collect::<Result<Vec<_>, _>>()?;
    for _ in &declared_dynamic_parameters {
        expected_parameters.push(ValueShape::integer(pointer_size, pointer_alignment));
        expected_parameters.push(ValueShape::integer(pointer_size, pointer_alignment));
    }
    let expected = evaluate_call_plan(
        CallingPolicy::native_for_target(native),
        &CallSignature {
            parameters: expected_parameters,
            result,
        },
    )
    .map_err(|_| invalid.clone())?;
    // The signature roster binds each declared descriptor to its exact
    // `{instance, table}` placements in the evaluated plan.
    let descriptor_base = abstracted.parameters.len();
    if target.graph.dynamic_parameters.len() != declared_dynamic_parameters.len()
        || target
            .graph
            .dynamic_parameters
            .iter()
            .enumerate()
            .any(|(index, abi)| {
                abi.parameter != *declared_dynamic_parameters[index]
                    || abi.instance != expected.parameters[descriptor_base + index * 2]
                    || abi.table != expected.parameters[descriptor_base + index * 2 + 1]
            })
    {
        return Err(invalid);
    }
    if expected
        .result
        .as_ref()
        .is_some_and(|value| !scalar_register(value))
    {
        return Err(invalid);
    }
    match &abstracted.result {
        AbstractFunctionResult::Scalar(result) => {
            if let Some(abi) = &target.scalar_abi {
                // A retained `scalar_abi` is only emitted when the function
                // declares no descriptor parameters.
                if !declared_dynamic_parameters.is_empty()
                    || abi.call_plan != expected
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
                    return Err(invalid);
                }
            } else {
                // Descriptor-parameter scalar functions carry their signature
                // in the graph header: the evaluated call plan plus the
                // roster rows bound above, with no separate `scalar_abi`.
                if declared_dynamic_parameters.is_empty()
                    || target.graph.call_plan != expected
                    || target.graph.scalar_parameters.len() != abstracted.parameters.len()
                    || target
                        .graph
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
                return Err(invalid);
            }
        }
        _ => {
            return Err(invalid);
        }
    }
    Ok(expected)
}
