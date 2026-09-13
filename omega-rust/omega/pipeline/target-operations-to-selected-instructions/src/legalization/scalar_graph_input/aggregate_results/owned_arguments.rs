//! Owned aggregate actuals rejoin their exact incoming storage or dominating producer.
use super::*;

pub(super) fn reconstruct(
    argument: &terminal_psi::StructuralArgument,
    position: usize,
    call_operation: semantic_vocabulary::OperationId,
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
    if !argument.path.is_empty()
        || destination.access != terminal_psi::StructuralAccess::Owned
        || destination.multiplicity == StructuralMultiplicity::Linear
        || destination.is_self
        || !destination.qualifications.is_empty()
        || !destination.projected_qualifications.is_empty()
    {
        return Err(invalid);
    }
    let shape =
        crate::structural_reference_input::parameter_shape(destination, &plan.structural_types)
            .ok_or(invalid.clone())?;
    let value_shape = crate::structural_reference_input::owned_aggregate_shape(
        destination.structural_type,
        &plan.structural_types,
    )
    .ok_or(invalid.clone())?;
    if shape != value_shape {
        return Err(invalid);
    }
    let source = if let Some(parameter) = caller
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == argument.place)
    {
        if parameter.structural_type != destination.structural_type
            || parameter.access != destination.access
            || parameter.multiplicity != destination.multiplicity
            || parameter.is_self
            || !parameter.qualifications.is_empty()
            || !parameter.projected_qualifications.is_empty()
        {
            return Err(invalid);
        }
        let retained = native
            .functions
            .iter()
            .find(|function| function.machine == caller.machine)
            .and_then(super::structural_parameters)
            .and_then(|parameters| {
                parameters
                    .iter()
                    .find(|retained| retained.place == argument.place)
            })
            .ok_or(invalid.clone())?;
        if retained.structural_type != parameter.structural_type
            || retained.access != parameter.access
            || retained.multiplicity != parameter.multiplicity
            || retained.shape != shape
            || retained.placement.shape != shape
            || !retained.projected_qualifications.is_empty()
        {
            return Err(invalid);
        }
        retained.placement.clone().into()
    } else {
        let (producer, result) = super::structural_case::source_result(caller, argument.place)?;
        if result.structural_type != destination.structural_type
            || result.multiplicity != destination.multiplicity
            || !result.claims.is_empty()
            || !result.qualifications.is_empty()
            || !result.projected_qualifications.is_empty()
        {
            return Err(invalid);
        }
        let producer_site = caller
            .blocks
            .iter()
            .find_map(|block| {
                block.nodes.iter().enumerate().find_map(|(position, node)| {
                    matches!(&node.operation,
                AbstractOperation::EstablishScalarArray { psi_operation, result: actual, .. }
                | AbstractOperation::EstablishRecord { psi_operation, result: actual, .. }
                | AbstractOperation::EstablishScalarCase { psi_operation, result: actual, .. }
                | AbstractOperation::CallStructural { psi_operation, result: actual, .. }
                if *psi_operation == producer && actual == result)
                    .then_some((block.id, position))
                })
            })
            .ok_or(invalid.clone())?;
        let call_site =
            caller
                .blocks
                .iter()
                .find_map(|block| {
                    block.nodes.iter().enumerate().find_map(|(position, node)| {
                        match &node.operation {
                            AbstractOperation::CallStructural {
                                psi_operation,
                                structural_arguments,
                                ..
                            }
                            | AbstractOperation::CallUnit {
                                psi_operation,
                                structural_arguments,
                                ..
                            }
                            | AbstractOperation::CallStructuralScalar {
                                psi_operation,
                                structural_arguments,
                                ..
                            } if *psi_operation == call_operation
                                && structural_arguments.contains(argument) =>
                            {
                                Some((block.id, position))
                            }
                            _ => None,
                        }
                    })
                })
                .ok_or(invalid.clone())?;
        if (producer_site.0 == call_site.0 && producer_site.1 >= call_site.1)
            || (producer_site.0 != call_site.0
                && !super::target::control_flow::sources::dominates(
                    caller,
                    producer_site.0,
                    call_site.0,
                ))
            || !caller.structural_places.iter().any(|place| {
                place.id == result.place
                    && place.kind
                        == StructuralPlaceKind::OperationResult {
                            producer,
                            structural_type: result.structural_type,
                        }
            })
        {
            return Err(invalid);
        }
        target_operations::TargetStructuralArgumentSource::StructuralHome {
            psi_operation: producer,
        }
    };
    let placement = call
        .parameters
        .get(callee.parameters.len() + position)
        .ok_or(invalid.clone())?;
    if placement.shape != shape {
        return Err(invalid);
    }
    Ok(target_operations::TargetStructuralArgument {
        place: argument.place,
        access: argument.access,
        path: Vec::new(),
        root_structural_type: destination.structural_type,
        structural_type: destination.structural_type,
        shape,
        source_byte_offset: 0,
        fixed_array_length: None,
        element_stride: None,
        source,
        destination: placement.clone(),
    })
}
