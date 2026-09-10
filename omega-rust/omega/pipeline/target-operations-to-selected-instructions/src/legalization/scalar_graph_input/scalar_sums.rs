//! Independent scalar-sum layout and ordinary graph signature custody.
use super::*;
use semantic_vocabulary::{PlaceId, StructuralPlaceKind};
use terminal_psi::{StructuralMultiplicity, StructuralOperationResult, StructuralTypeShape};

pub(super) fn uses(function: &PsiOptimizationFunction) -> bool {
    function.result.structural().is_some()
        || function
            .blocks
            .iter()
            .flat_map(|block| &block.nodes)
            .any(|node| {
                matches!(
                    node.operation,
                    AbstractOperation::EstablishScalarCase { .. }
                        | AbstractOperation::CallStructural { .. }
                )
            })
}

pub(in crate::legalization) fn layout(
    result: &StructuralOperationResult,
    plan: &AbstractOperationPlan,
) -> Result<calling_conventions::ConventionalSumLayout, LegalizationError> {
    let invalid = LegalizationError::SourceCustodyMismatch;
    if result.multiplicity == StructuralMultiplicity::Linear
        || !result.claims.is_empty()
        || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
    {
        return Err(invalid);
    }
    let mut declarations = plan
        .structural_types
        .iter()
        .filter(|declaration| declaration.id == result.structural_type);
    let declaration = declarations.next().ok_or(invalid.clone())?;
    let StructuralTypeShape::Sum { cases } = &declaration.shape else {
        return Err(invalid);
    };
    if declarations.next().is_some() {
        return Err(invalid);
    }
    let payloads = cases
        .iter()
        .map(|case| {
            case.fields
                .iter()
                .map(|field| {
                    if field.relevance.is_erased() {
                        return Err(invalid.clone());
                    }
                    let Some(ScalarType::Integer(integer)) = field.field_type.scalar_type() else {
                        return Err(invalid.clone());
                    };
                    scalar_shape(ScalarType::Integer(integer)).ok_or(invalid.clone())
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .collect::<Result<Vec<_>, _>>()?;
    calling_conventions::evaluate_conventional_sum_layout(&[], &payloads).map_err(|_| invalid)
}

pub(super) fn roster(function: &PsiOptimizationFunction) -> bool {
    function.declared_places
        == function
            .structural_places
            .iter()
            .map(|place| place.id)
            .collect::<std::collections::BTreeSet<_>>()
        && function.structural_places.iter().all(|place| {
            if place.kind == StructuralPlaceKind::Result {
                return function
                    .result
                    .structural()
                    .is_some_and(|result| result.place == place.id);
            }
            if let Ok((producer, result)) =
                super::structural_case::source_result(function, place.id)
            {
                return place.kind
                    == StructuralPlaceKind::OperationResult {
                        producer,
                        structural_type: result.structural_type,
                    };
            }
            function.structural_parameters.iter().any(|parameter| {
                parameter.place == place.id
                    && place.kind
                        == StructuralPlaceKind::Parameter {
                            position: parameter.position,
                            is_self: parameter.is_self,
                        }
            }) || function.blocks.iter().any(|block| {
                block.structural_parameters.iter().any(|parameter| {
                    parameter.place == place.id
                        && place.kind
                            == StructuralPlaceKind::BlockParameter {
                                block: block.id,
                                position: parameter.position,
                            }
                })
            })
        })
}

pub(super) fn cleanup(
    function: &PsiOptimizationFunction,
    actions: &[terminal_psi::TerminalAffineCleanupAction],
) -> bool {
    let mut discarded = std::collections::BTreeSet::new();
    actions.iter().all(|action| {
        let terminal_psi::TerminalAffineCleanupAction::DiscardRoot(place) = action else {
            return false;
        };
        discarded.insert(*place)
            && super::structural_case::source_result(function, *place).is_ok_and(|(_, result)| {
                result.multiplicity == StructuralMultiplicity::Affine
                    && result.claims.is_empty()
                    && result.qualifications.is_empty()
                    && result.projected_qualifications.is_empty()
            })
    })
}

pub(super) fn header(
    target: &TargetFunction,
    abstracted: &AbstractFunction,
    optimized: &PsiOptimizationFunction,
    native: ::target::NativeTarget,
    plan: &AbstractOperationPlan,
) -> Result<CallPlan, LegalizationError> {
    let invalid = LegalizationError::SourceCustodyMismatch;
    let TargetOperation::ControlGraph(graph) = &target.operation else {
        return Err(invalid);
    };
    if target.machine != abstracted.machine
        || target.machine != optimized.machine
        || target.attachment != abstracted.attachment
        || target.attachment != optimized.attachment
        || target.scalar_abi.is_some()
        || target.mixed_structural_scalar_abi.is_some()
        || optimized.result != abstracted.result
        || !roster(optimized)
        || optimized.structural_parameters != abstracted.structural_parameters
        || !abstracted.entry_claims.is_empty()
        || !optimized.entry_claims.is_empty()
        || !optimized.entry_claim_declarations.is_empty()
        || !optimized.content_entry_claims.is_empty()
        || abstracted.published_service_ceiling != optimized.published_service_ceiling
        || abstracted.parameters.len() != optimized.parameters.len()
        || graph.scalar_parameters.len() != abstracted.parameters.len()
        || graph.parameters.len() != abstracted.structural_parameters.len()
    {
        return Err(invalid);
    }
    let result = match &abstracted.result {
        AbstractFunctionResult::Unit => None,
        AbstractFunctionResult::Structural(result) => Some(
            if plan.structural_types.iter().any(|declaration| {
                declaration.id == result.structural_type
                    && matches!(declaration.shape, StructuralTypeShape::Sum { .. })
            }) {
                layout(
                    &StructuralOperationResult {
                        place: result.place,
                        structural_type: result.structural_type,
                        multiplicity: result.multiplicity,
                        qualifications: result.qualifications.clone(),
                        projected_qualifications: result.projected_qualifications.clone(),
                        claims: Vec::new(),
                    },
                    plan,
                )?
                .shape
            } else {
                if result.multiplicity == StructuralMultiplicity::Linear
                    || !result.qualifications.is_empty()
                    || !result.projected_qualifications.is_empty()
                {
                    return Err(invalid);
                }
                crate::structural_reference_input::shape(
                    result.structural_type,
                    &plan.structural_types,
                )
                .ok_or(invalid.clone())?
            },
        ),
        _ => return Err(invalid),
    };
    let mut shapes = abstracted
        .parameters
        .iter()
        .map(|parameter| scalar_shape(parameter.scalar_type).ok_or(invalid.clone()))
        .collect::<Result<Vec<_>, _>>()?;
    for parameter in &abstracted.structural_parameters {
        shapes.push(
            crate::structural_reference_input::parameter_shape(parameter, &plan.structural_types)
                .ok_or(invalid.clone())?,
        );
    }
    let expected = evaluate_call_plan(
        CallingPolicy::native_for_target(native),
        &CallSignature {
            parameters: shapes,
            result,
        },
    )
    .map_err(|_| invalid.clone())?;
    if graph.call_plan != expected {
        return Err(invalid);
    }
    for (position, ((declared, actual), retained)) in abstracted
        .parameters
        .iter()
        .zip(&optimized.parameters)
        .zip(&graph.scalar_parameters)
        .enumerate()
    {
        if actual.value != declared.value
            || actual.scalar_type != declared.scalar_type
            || actual.site != ValueDefinitionSite::FunctionParameter(position as u32)
            || retained.value != declared.value
            || retained.scalar_type != declared.scalar_type
            || retained.placement != expected.parameters[position]
        {
            return Err(invalid);
        }
    }
    for (position, (declared, retained)) in abstracted
        .structural_parameters
        .iter()
        .zip(&graph.parameters)
        .enumerate()
    {
        if declared.position as usize != position
            || retained.place != declared.place
            || retained.structural_type != declared.structural_type
            || retained.access != declared.access
            || retained.multiplicity != declared.multiplicity
            || !retained.projected_qualifications.is_empty()
            || retained.shape != expected.parameters[abstracted.parameters.len() + position].shape
            || retained.placement != expected.parameters[abstracted.parameters.len() + position]
        {
            return Err(invalid);
        }
    }
    if optimized
        .blocks
        .iter()
        .flat_map(|block| &block.structural_parameters)
        .any(|parameter| !byte_parameter(parameter, plan))
    {
        return Err(invalid);
    }
    Ok(expected)
}

fn byte_parameter(
    parameter: &terminal_psi::StructuralParameterDeclaration,
    plan: &AbstractOperationPlan,
) -> bool {
    !parameter.is_self
        && parameter.multiplicity == StructuralMultiplicity::Unrestricted
        && matches!(
            parameter.access,
            terminal_psi::StructuralAccess::SharedBorrow
                | terminal_psi::StructuralAccess::MutableBorrow
        )
        && parameter.qualifications.is_empty()
        && parameter.projected_qualifications.is_empty()
        && plan.structural_types.iter().any(|declaration| {
            declaration.id == parameter.structural_type
                && matches!(
                    declaration.shape,
                    StructuralTypeShape::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BorrowedView
                    )
                )
        })
}

pub(in crate::legalization) fn call_argument(
    argument: &terminal_psi::StructuralArgument,
    position: usize,
    caller: &PsiOptimizationFunction,
    callee: &PsiOptimizationFunction,
    call: &CallPlan,
    native: &TargetOperationPlan,
    plan: &AbstractOperationPlan,
) -> Result<target_operations::TargetStructuralArgument, LegalizationError> {
    let invalid = LegalizationError::SourceCustodyMismatch;
    let destination = callee
        .structural_parameters
        .get(position)
        .ok_or(invalid.clone())?;
    let caller_target = native
        .functions
        .iter()
        .find(|function| function.machine == caller.machine)
        .ok_or(invalid.clone())?;
    let (source, binding) = if let Some(source) = caller
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == argument.place)
    {
        let retained = super::structural_parameters(caller_target)
            .and_then(|parameters| {
                parameters
                    .iter()
                    .find(|parameter| parameter.place == argument.place)
            })
            .ok_or(invalid.clone())?;
        (source, retained.placement.clone().into())
    } else {
        let (block, source) = caller
            .blocks
            .iter()
            .find_map(|block| {
                block
                    .structural_parameters
                    .iter()
                    .find(|parameter| parameter.place == argument.place)
                    .map(|parameter| (block.id, parameter))
            })
            .ok_or(invalid.clone())?;
        (
            source,
            target_operations::TargetStructuralArgumentSource::BlockParameter {
                block,
                place: source.place,
            },
        )
    };
    if !argument.path.is_empty()
        || !byte_parameter(source, plan)
        || !byte_parameter(destination, plan)
        || source.structural_type != destination.structural_type
        || source.access != argument.access
        || argument.access != destination.access
    {
        return Err(invalid);
    }
    let placement = call
        .parameters
        .get(callee.parameters.len() + position)
        .ok_or(invalid.clone())?;
    if placement.shape != ValueShape::borrowed_reference(16, 8) {
        return Err(invalid);
    }
    Ok(target_operations::TargetStructuralArgument {
        place: argument.place,
        access: argument.access,
        path: Vec::new(),
        root_structural_type: source.structural_type,
        structural_type: source.structural_type,
        shape: placement.shape,
        source_byte_offset: 0,
        fixed_array_length: None,
        element_stride: None,
        source: binding,
        destination: placement.clone(),
    })
}

pub(super) fn result_home(
    function: &PsiOptimizationFunction,
    place: PlaceId,
    plan: &AbstractOperationPlan,
) -> Result<target_operations::TargetStructuralHomeRequirement, LegalizationError> {
    let (operation, result) = super::structural_case::source_result(function, place)?;
    Ok(target_operations::TargetStructuralHomeRequirement {
        defining_operation: operation,
        result: result.clone(),
        layout: target_operations::TargetStructuralHomeLayout::Sum(layout(result, plan)?),
    })
}
