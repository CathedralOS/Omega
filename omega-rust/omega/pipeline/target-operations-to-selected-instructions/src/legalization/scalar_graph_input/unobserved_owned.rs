//! Independent input replay for whole owned values with no runtime observers.
use super::*;
use terminal_psi::{
    StructuralAccess, StructuralMultiplicity, StructuralParameterDeclaration,
    TerminalAffineCleanupAction,
};

fn plain(parameter: &StructuralParameterDeclaration) -> bool {
    parameter.access == StructuralAccess::Owned
        && matches!(
            parameter.multiplicity,
            StructuralMultiplicity::Affine | StructuralMultiplicity::Unrestricted
        )
        && !parameter.is_self
        && parameter.qualifications.is_empty()
        && parameter.projected_qualifications.is_empty()
}

pub(super) fn cleanup(
    function: &PsiOptimizationFunction,
    actions: &[TerminalAffineCleanupAction],
) -> bool {
    let mut seen = std::collections::BTreeSet::new();
    actions.iter().all(|action| match action {
        TerminalAffineCleanupAction::DiscardRoot(place) => {
            seen.insert(*place)
                && function
                    .structural_parameters
                    .iter()
                    .chain(
                        function
                            .blocks
                            .iter()
                            .flat_map(|block| &block.structural_parameters),
                    )
                    .any(|parameter| {
                        parameter.place == *place
                            && plain(parameter)
                            && parameter.multiplicity == StructuralMultiplicity::Affine
                    })
        }
        _ => false,
    })
}

pub(super) fn body(function: &PsiOptimizationFunction) -> bool {
    !function.structural_parameters.is_empty()
        && function.attachment.is_none()
        && function.entry_claims.is_empty()
        && function.entry_claim_declarations.is_empty()
        && function.content_entry_claims.is_empty()
        && function.published_service_ceiling.is_empty()
        && function
            .structural_parameters
            .iter()
            .chain(
                function
                    .blocks
                    .iter()
                    .flat_map(|block| &block.structural_parameters),
            )
            .all(plain)
        && function
            .blocks
            .iter()
            .flat_map(|block| &block.nodes)
            .all(|node| match &node.operation {
                AbstractOperation::IntegerConstant { .. }
                | AbstractOperation::BooleanConstant { .. }
                | AbstractOperation::IntegerWiden { .. }
                | AbstractOperation::IntegerEqual { .. }
                | AbstractOperation::IntegerLessThan { .. }
                | AbstractOperation::IntegerLessOrEqual { .. }
                | AbstractOperation::ExactIntegerAdd { .. }
                | AbstractOperation::ExactIntegerSubtract { .. }
                | AbstractOperation::Call { .. } => true,
                AbstractOperation::Return {
                    cleanup_actions, ..
                }
                | AbstractOperation::ReturnUnit {
                    cleanup_actions, ..
                } => cleanup(function, cleanup_actions),
                AbstractOperation::Jump {
                    trivial_affine_discards,
                    residual_affine_discards,
                    ..
                } => trivial_affine_discards.is_empty() && residual_affine_discards.is_empty(),
                AbstractOperation::Conditional {
                    when_true,
                    when_false,
                    ..
                } => {
                    when_true.trivial_affine_discards.is_empty()
                        && when_false.trivial_affine_discards.is_empty()
                }
                _ => false,
            })
}

pub(super) fn validate(
    target: &TargetFunction,
    abstracted: &AbstractFunction,
    function: &PsiOptimizationFunction,
    native: ::target::NativeTarget,
    plan: &AbstractOperationPlan,
) -> Result<CallPlan, LegalizationError> {
    let invalid = LegalizationError::SourceCustodyMismatch;
    let TargetOperation::ControlGraph(graph) = &target.operation else {
        return Err(invalid);
    };
    if !body(function)
        || target.machine != abstracted.machine
        || target.machine != function.machine
        || target.attachment != function.attachment
        || abstracted.attachment != function.attachment
        || abstracted.structural_parameters != function.structural_parameters
        || abstracted.result != function.result
        || !abstracted.entry_claims.is_empty()
        || abstracted.published_service_ceiling != function.published_service_ceiling
        || target.scalar_abi.is_some()
        || graph.structural_types != plan.structural_types
        || graph.parameters.len() != function.structural_parameters.len()
        || graph.scalar_parameters.len() != abstracted.parameters.len()
        || function.parameters.len() != abstracted.parameters.len()
    {
        return Err(invalid);
    }
    let mut shapes = abstracted
        .parameters
        .iter()
        .map(|parameter| scalar_shape(parameter.scalar_type).ok_or(invalid.clone()))
        .collect::<Result<Vec<_>, _>>()?;
    let scalar_count = shapes.len();
    for (position, (parameter, retained)) in function
        .structural_parameters
        .iter()
        .zip(&graph.parameters)
        .enumerate()
    {
        let shape = crate::structural_reference_input::shape(
            parameter.structural_type,
            &plan.structural_types,
        )
        .ok_or(invalid.clone())?;
        if parameter.position as usize != position
            || retained.place != parameter.place
            || retained.structural_type != parameter.structural_type
            || retained.access != parameter.access
            || retained.multiplicity != parameter.multiplicity
            || retained.projected_qualifications != parameter.projected_qualifications
            || retained.shape != shape
        {
            return Err(invalid);
        }
        shapes.push(shape);
    }
    let result_shape = match function.result {
        AbstractFunctionResult::Scalar(result)
            if result.scalar_type == ScalarType::Integer(u64_type()) =>
        {
            Some(ValueShape::integer(8, 8))
        }
        AbstractFunctionResult::Unit => None,
        _ => return Err(invalid),
    };
    let expected = evaluate_call_plan(
        CallingPolicy::native_for_target(native),
        &CallSignature {
            parameters: shapes,
            result: result_shape,
        },
    )
    .map_err(|_| invalid.clone())?;
    if graph.call_plan != expected {
        return Err(invalid);
    }
    for (position, ((declared, current), retained)) in abstracted
        .parameters
        .iter()
        .zip(&function.parameters)
        .zip(&graph.scalar_parameters)
        .enumerate()
    {
        if declared.value != current.value
            || declared.scalar_type != current.scalar_type
            || current.site != ValueDefinitionSite::FunctionParameter(position as u32)
            || retained.value != declared.value
            || retained.scalar_type != declared.scalar_type
            || retained.placement != expected.parameters[position]
        {
            return Err(invalid);
        }
    }
    if graph
        .parameters
        .iter()
        .zip(&expected.parameters[scalar_count..])
        .any(|(parameter, placement)| parameter.placement != *placement)
    {
        return Err(invalid);
    }
    match function.result {
        AbstractFunctionResult::Scalar(result) => {
            let abi = target
                .mixed_structural_scalar_abi
                .as_ref()
                .ok_or(invalid.clone())?;
            if abi.call_plan != expected
                || abi.scalar_parameters != graph.scalar_parameters
                || abi.structural_parameters != graph.parameters
                || abi.result.value != result.value
                || abi.result.scalar_type != result.scalar_type
                || Some(&abi.result.placement) != expected.result.as_ref()
            {
                return Err(invalid);
            }
        }
        AbstractFunctionResult::Unit if target.mixed_structural_scalar_abi.is_none() => {}
        _ => return Err(invalid),
    }
    let declarations = function
        .structural_parameters
        .iter()
        .chain(
            function
                .blocks
                .iter()
                .flat_map(|block| &block.structural_parameters),
        )
        .collect::<Vec<_>>();
    if declarations.iter().any(|parameter| {
        !crate::unobserved_owned_input::plain_type(
            parameter.structural_type,
            &plan.structural_types,
        )
    }) {
        return Err(invalid);
    }
    if function.structural_places.len() != declarations.len()
        || function.declared_places
            != function
                .structural_places
                .iter()
                .map(|place| place.id)
                .collect()
    {
        return Err(invalid);
    }
    for place in &function.structural_places {
        let invocation = function.structural_parameters.iter().any(|parameter| {
            parameter.place == place.id
                && place.kind
                    == (semantic_vocabulary::StructuralPlaceKind::Parameter {
                        position: parameter.position,
                        is_self: false,
                    })
        });
        let arrival = function.blocks.iter().any(|block| {
            block
                .structural_parameters
                .iter()
                .enumerate()
                .any(|(position, parameter)| {
                    parameter.place == place.id
                        && parameter.position as usize == position
                        && place.kind
                            == (semantic_vocabulary::StructuralPlaceKind::BlockParameter {
                                block: block.id,
                                position: parameter.position,
                            })
                })
        });
        if !invocation && !arrival {
            return Err(invalid);
        }
    }
    for edge in function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .flat_map(|node| &node.successors)
    {
        let destination = function
            .blocks
            .iter()
            .find(|block| block.id == edge.target)
            .ok_or(invalid.clone())?;
        if edge.structural_bindings.len() != destination.structural_parameters.len() {
            return Err(invalid);
        }
        for (binding, parameter) in edge
            .structural_bindings
            .iter()
            .zip(&destination.structural_parameters)
        {
            let source = declarations
                .iter()
                .find(|source| source.place == binding.argument.place)
                .ok_or(invalid.clone())?;
            if binding.parameter != parameter.place
                || binding.argument.access != StructuralAccess::Owned
                || !binding.argument.path.is_empty()
                || source.structural_type != parameter.structural_type
                || source.multiplicity != parameter.multiplicity
            {
                return Err(invalid);
            }
        }
    }
    Ok(expected)
}
