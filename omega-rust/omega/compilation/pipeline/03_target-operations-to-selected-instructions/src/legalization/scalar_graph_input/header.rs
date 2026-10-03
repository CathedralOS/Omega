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
    if target.machine != abstracted.machine
        || target.machine != optimized.machine
        || target.attachment != abstracted.attachment
        || target.attachment != optimized.attachment
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
    // Borrowed descriptor parameters are zero-code leading declarations; each
    // occupies a trailing `{instance, table}` pointer pair whose dense lane
    // ordinal and borrowed access the declaration row must retain exactly.
    let declared_dynamic_parameters = descriptor_parameters(abstracted).collect::<Vec<_>>();
    let pointer_size =
        u16::try_from(native.pointer_size).map_err(|_| LegalizationError::custody())?;
    let pointer_alignment =
        u16::try_from(native.pointer_alignment).map_err(|_| LegalizationError::custody())?;
    let mut expected_parameters = abstracted
        .parameters
        .iter()
        .map(|parameter| scalar_shape(parameter.scalar_type).ok_or(LegalizationError::custody()))
        .collect::<Result<Vec<_>, _>>()?;
    for _ in &declared_dynamic_parameters {
        expected_parameters.push(ValueShape::integer(
            u32::from(pointer_size),
            pointer_alignment,
        ));
        expected_parameters.push(ValueShape::integer(
            u32::from(pointer_size),
            pointer_alignment,
        ));
    }
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
                    return Err(LegalizationError::custody());
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
                    return Err(LegalizationError::custody());
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
                return Err(LegalizationError::custody());
            }
        }
        _ => {
            return Err(LegalizationError::custody());
        }
    }
    Ok(expected)
}

/// Every function entry replays its descriptor roster, regardless of its
/// result carrier. Checking only calls or scalar signatures would admit
/// fabricated descriptors on structural functions and lose unused declarations.
pub(super) fn validate_descriptor_parameters(
    native: ::target::NativeTarget,
    target: &TargetFunction,
    abstracted: &AbstractFunction,
    call_plan: &CallPlan,
) -> Result<(), LegalizationError> {
    let declared = descriptor_parameters(abstracted).collect::<Vec<_>>();
    if target.graph.dynamic_parameters.len() != declared.len() {
        return Err(LegalizationError::custody());
    }
    let descriptor_base = abstracted
        .parameters
        .len()
        .checked_add(abstracted.structural_parameters.len())
        .ok_or_else(LegalizationError::custody)?;
    let pointer_alignment =
        u16::try_from(native.pointer_alignment).map_err(|_| LegalizationError::custody())?;
    let pointer_size =
        u32::try_from(native.pointer_size).map_err(|_| LegalizationError::custody())?;
    let pointer_shape = ValueShape::integer(pointer_size, pointer_alignment);
    for (parameter_index, (parameter, abi)) in declared
        .iter()
        .zip(&target.graph.dynamic_parameters)
        .enumerate()
    {
        let source_position = descriptor_base
            .checked_add(parameter_index)
            .and_then(|position| u32::try_from(position).ok())
            .ok_or_else(LegalizationError::custody)?;
        let instance_position = parameter_index
            .checked_mul(2)
            .and_then(|position| descriptor_base.checked_add(position))
            .ok_or_else(LegalizationError::custody)?;
        let table_position = instance_position
            .checked_add(1)
            .ok_or_else(LegalizationError::custody)?;
        if parameter.owner != abstracted.machine
            || usize::try_from(parameter.ordinal).ok() != Some(parameter_index)
            || parameter.source_position != source_position
            || !matches!(
                parameter.access,
                terminal_psi::StructuralAccess::SharedBorrow
                    | terminal_psi::StructuralAccess::MutableBorrow
            )
            || abi.parameter != **parameter
            || abi.instance.shape != pointer_shape
            || abi.table.shape != pointer_shape
            || call_plan.parameters.get(instance_position) != Some(&abi.instance)
            || call_plan.parameters.get(table_position) != Some(&abi.table)
        {
            return Err(LegalizationError::custody());
        }
    }
    Ok(())
}

fn descriptor_parameters(
    abstracted: &AbstractFunction,
) -> impl Iterator<Item = &terminal_psi::TerminalDynamicDescriptorParameter> {
    abstracted
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
}
