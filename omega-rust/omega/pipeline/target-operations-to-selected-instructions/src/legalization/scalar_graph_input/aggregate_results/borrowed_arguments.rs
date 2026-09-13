//! Rejoin aggregate references to exact incoming, operation, or block-entry storage.
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
    if argument.access != destination.access
        || !matches!(
            argument.access,
            terminal_psi::StructuralAccess::SharedBorrow
                | terminal_psi::StructuralAccess::MutableBorrow
        )
        || destination.multiplicity != StructuralMultiplicity::Unrestricted
        || !destination.qualifications.is_empty()
        || !destination.projected_qualifications.is_empty()
    {
        return Err(invalid);
    }
    let (root, source) = if let Some(parameter) = caller
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == argument.place)
    {
        if !matches!(
            parameter.access,
            terminal_psi::StructuralAccess::Owned | terminal_psi::StructuralAccess::MutableBorrow
        ) && parameter.access != argument.access
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
                    .find(|value| value.place == argument.place)
            })
            .ok_or(invalid.clone())?;
        (parameter.structural_type, retained.placement.clone().into())
    } else {
        let site = |operation| {
            caller.blocks.iter().find_map(|block| {
                block
                    .nodes
                    .iter()
                    .position(|node| {
                        matches!(&node.operation,
                AbstractOperation::EstablishRecord { psi_operation, .. }
                | AbstractOperation::EstablishScalarCase { psi_operation, .. }
                | AbstractOperation::CallStructural { psi_operation, .. }
                | AbstractOperation::CallStructuralScalar { psi_operation, .. }
                | AbstractOperation::CallUnit { psi_operation, .. } if *psi_operation == operation)
                    })
                    .map(|position| (block.id, position))
            })
        };
        let after = site(call_operation).ok_or(invalid.clone())?;
        match super::structural_case::source_owner(caller, argument.place)? {
            legalized_operations::LegalizedStructuralCaseSource::OperationResult {
                operation,
                result,
            } => {
                home_layout(&result, plan)?;
                let before = site(operation).ok_or(invalid.clone())?;
                if (before.0 == after.0 && before.1 >= after.1)
                    || (before.0 != after.0
                        && !super::target::control_flow::sources::dominates(
                            caller, before.0, after.0,
                        ))
                {
                    return Err(invalid);
                }
                (
                    result.structural_type,
                    target_operations::TargetStructuralArgumentSource::StructuralHome {
                        psi_operation: operation,
                    },
                )
            }
            legalized_operations::LegalizedStructuralCaseSource::BlockParameter {
                block,
                declaration,
            } => {
                if argument.access != terminal_psi::StructuralAccess::SharedBorrow
                    || !super::target::control_flow::sources::dominates(caller, block, after.0)
                    || !caller.structural_places.iter().any(|place| {
                        place.id == declaration.place
                            && place.kind
                                == StructuralPlaceKind::BlockParameter {
                                    block,
                                    position: declaration.position,
                                }
                    })
                {
                    return Err(invalid);
                }
                (
                    declaration.structural_type,
                    target_operations::TargetStructuralArgumentSource::BlockParameter {
                        block,
                        place: declaration.place,
                    },
                )
            }
        }
    };
    let (selected, offset) =
        crate::structural_reference_input::project(root, &argument.path, &plan.structural_types)
            .ok_or(invalid.clone())?;
    let referent = crate::structural_reference_input::shape(selected, &plan.structural_types)
        .ok_or(invalid.clone())?;
    let shape = ValueShape::borrowed_reference(referent.byte_size, referent.alignment);
    let placement = call
        .parameters
        .get(callee.parameters.len() + position)
        .ok_or(invalid.clone())?;
    if selected != destination.structural_type || placement.shape != shape {
        return Err(invalid);
    }
    Ok(target_operations::TargetStructuralArgument {
        place: argument.place,
        access: argument.access,
        path: argument.path.clone(),
        root_structural_type: root,
        structural_type: selected,
        shape,
        source_byte_offset: offset,
        fixed_array_length: None,
        element_stride: None,
        source,
        destination: placement.clone(),
    })
}
