//! Ordinary Unit-call assignment and structural custody replay.

use super::*;

mod affine_projection;

fn exact_write_only_projection(
    body: &target_operations::TargetUnitBody,
    source: &target_operations::TargetStructuralParameter,
    argument: &target_operations::TargetStructuralArgument,
) -> bool {
    let Some(first_index) = argument
        .path
        .iter()
        .position(|segment| matches!(segment, terminal_psi::StructuralPathSegment::FixedIndex(_)))
    else {
        return false;
    };
    if argument.access != terminal_psi::StructuralAccess::WriteOnlyBorrow
        || source.access != terminal_psi::StructuralAccess::WriteOnlyBorrow
        || source.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
        || argument.fixed_array_length.is_some()
        || argument.element_stride.is_some()
        || !argument.path[..first_index].iter().all(|segment| {
            matches!(segment,
                terminal_psi::StructuralPathSegment::Field(identity) if !identity.is_empty())
        })
        || !argument.path[first_index..]
            .iter()
            .all(|segment| matches!(segment, terminal_psi::StructuralPathSegment::FixedIndex(_)))
    {
        return false;
    }
    let Some(declarations) = structural_scalar::declaration_map(&body.structural_types) else {
        return false;
    };
    let Some((leaf_type, leaf_shape, byte_offset)) = structural_scalar::resolve_projection_path(
        source.structural_type,
        &argument.path,
        &declarations,
    ) else {
        return false;
    };
    let Some(root_shape) =
        structural_scalar::structural_value_shape(source.structural_type, &declarations)
    else {
        return false;
    };
    matches!(
        declarations
            .get(&leaf_type)
            .map(|declaration| &declaration.shape),
        Some(terminal_psi::StructuralTypeShape::PrimitiveScalar(_))
    ) && argument.root_structural_type == source.structural_type
        && source.projected_qualifications.is_empty()
        && source.shape
            == ValueShape::borrowed_reference(root_shape.byte_size, root_shape.alignment)
        && argument.structural_type == leaf_type
        && argument.shape
            == ValueShape::borrowed_reference(leaf_shape.byte_size, leaf_shape.alignment)
        && argument.source_byte_offset == byte_offset
}

pub(super) fn assign(
    machine: MachineId,
    body: &target_operations::TargetUnitBody,
    operation: &TargetUnitOperation,
    preceding_operations: &[TargetUnitOperation],
    assigned_scalar_homes: &mut BTreeMap<ValueId, AssignedUnitScalarHome>,
    assigned_structural_homes: &BTreeMap<PlaceId, AssignedStructuralHome>,
    target: NativeTarget,
) -> Result<AssignedUnitOperation, AssignmentError> {
    let TargetUnitOperation::Call {
        psi_operation,
        callee,
        call_plan,
        scalar_arguments,
        arguments,
        claim_transfers,
        requirement_obligations,
        crash_continuations,
    } = operation
    else {
        unreachable!("ordinary call assignment received another operation role")
    };
    Ok({
        let invalid = || AssignmentError::UnitCallCustodyMismatch {
            machine,
            operation: *psi_operation,
        };
        let expected_call_plan = evaluate_call_plan(
            CallingPolicy::native_for_target(target),
            &CallSignature {
                parameters: scalar_arguments
                    .iter()
                    .map(|argument| argument.placement.shape)
                    .chain(arguments.iter().map(|argument| argument.shape))
                    .collect(),
                result: None,
            },
        )
        .map_err(|_| invalid())?;
        let scalar_count = scalar_arguments.len();
        if *call_plan != expected_call_plan
            || call_plan.parameters.len() != scalar_count + arguments.len()
            || scalar_arguments
                .iter()
                .enumerate()
                .any(|(index, argument)| {
                    usize::try_from(argument.parameter_index) != Ok(index)
                        || call_plan.parameters.get(index) != Some(&argument.placement)
                })
            || arguments
                .iter()
                .zip(&call_plan.parameters[scalar_count..])
                .any(|(argument, placement)| argument.destination != *placement)
        {
            return Err(invalid());
        }
        let assigned_scalar_arguments = scalar_arguments
            .iter()
            .map(|argument| {
                let source = match argument.source {
                    TargetUnitScalarArgumentSource::Parameter {
                        parameter_index,
                        source_value,
                        scalar_type,
                    } => {
                        let index = usize::try_from(parameter_index).map_err(|_| invalid())?;
                        let parameter = body.scalar_parameters.get(index).ok_or_else(invalid)?;
                        let source_placement =
                            body.call_plan.parameters.get(index).ok_or_else(invalid)?;
                        let shape = match scalar_type {
                            semantic_vocabulary::ScalarType::Boolean => ValueShape::integer(1, 1),
                            semantic_vocabulary::ScalarType::Integer(integer) => {
                                super::scalar_call::fixed_integer_shape(source_value, integer)
                                    .map_err(|_| invalid())?
                            }
                            semantic_vocabulary::ScalarType::IeeeFloat(_) => return Err(invalid()),
                        };
                        if parameter.value != source_value
                            || parameter.scalar_type != scalar_type
                            || parameter.placement != *source_placement
                            || source_placement.shape != shape
                            || argument.placement.shape != shape
                        {
                            return Err(invalid());
                        }
                        let location = match source_placement.locations.as_slice() {
                            [
                                ValueLocation::Register {
                                    register,
                                    value_byte_offset: 0,
                                    byte_size,
                                },
                            ] if *byte_size == shape.byte_size => {
                                crate::assignment::placement::require_register_architecture(
                                    source_value,
                                    *register,
                                    target.architecture,
                                )?;
                                AssignedScalarLocation::Register(*register)
                            }
                            [
                                ValueLocation::Stack {
                                    stack_byte_offset,
                                    value_byte_offset: 0,
                                    byte_size,
                                    ..
                                },
                            ] if *byte_size == shape.byte_size => {
                                AssignedScalarLocation::IncomingStack {
                                    byte_offset: *stack_byte_offset,
                                }
                            }
                            _ => return Err(invalid()),
                        };
                        AssignedUnitScalarArgumentSource::Parameter {
                            parameter_index,
                            source_value,
                            scalar_type,
                            location,
                        }
                    }
                    source => super::scalar_call::assign_known_unit_scalar_source(
                        source,
                        preceding_operations,
                        assigned_scalar_homes,
                    )
                    .ok_or_else(invalid)?,
                };
                Ok(AssignedUnitScalarCallArgument {
                    parameter_index: argument.parameter_index,
                    source,
                    destination: super::scalar_call::assigned_unit_scalar_destination(
                        argument.source.source_value(),
                        &argument.placement,
                        target,
                    )?,
                })
            })
            .collect::<Result<Vec<_>, AssignmentError>>()?;
        if arguments.iter().any(|argument| {
                let parameter_source = body.parameters.iter().find(|parameter| {
                    parameter.place == argument.place
                        && parameter.structural_type == argument.root_structural_type
                        && parameter.shape == argument.source.shape
                        && parameter.placement == argument.source
                });
                let parameter_source = parameter_source.is_some_and(|parameter| {
                    if argument.path.iter().any(|segment| {
                        matches!(segment, terminal_psi::StructuralPathSegment::FixedIndex(_))
                    }) && argument.access == terminal_psi::StructuralAccess::WriteOnlyBorrow
                    {
                        exact_write_only_projection(body, parameter, argument)
                    } else if !argument.path.is_empty()
                        && argument.access == terminal_psi::StructuralAccess::Owned
                        && parameter.multiplicity == terminal_psi::StructuralMultiplicity::Affine
                    {
                        affine_projection::exact(body, parameter, argument)
                    } else {
                        true
                    }
                });
                let trivial_local_source = preceding_operations
                    .iter()
                    .filter_map(|preceding| match preceding {
                        TargetUnitOperation::EstablishTrivialAffineLocal {
                            psi_operation,
                            place,
                            structural_type,
                        } if place.id == argument.place => {
                            Some((*psi_operation, place, structural_type))
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                let exact_trivial_local = matches!(trivial_local_source.as_slice(), [(establishment, place, structural_type)]
                    if argument.path.is_empty()
                        && argument.access == terminal_psi::StructuralAccess::Owned
                        && argument.root_structural_type == structural_type.id
                        && argument.structural_type == structural_type.id
                        && argument.shape == ValueShape::integer(0, 1)
                        && argument.source.shape == argument.shape
                        && argument.source.locations.is_empty()
                        && matches!(
                            place.kind,
                            semantic_vocabulary::StructuralPlaceKind::TrivialAffineLocal {
                                structural_type: local_type,
                                construction: None,
                                ..
                            } if local_type == structural_type.id
                        )
                        && matches!(
                            structural_type.shape,
                            terminal_psi::StructuralTypeShape::Record { ref fields }
                                if fields.is_empty()
                        )
                        && body.structural_types.iter().any(|candidate| candidate == *structural_type)
                        && !preceding_operations.iter().any(|preceding| match preceding {
                            TargetUnitOperation::Call { arguments, .. }
                            | TargetUnitOperation::StructuralScalarCall { arguments, .. } => {
                                arguments.iter().any(|candidate| {
                                    candidate.place == argument.place
                                        && candidate.path.is_empty()
                                        && candidate.access == terminal_psi::StructuralAccess::Owned
                                })
                            }
                            _ => false,
                        })
                        && *establishment != *psi_operation);
                let affine_scalar_record_source = preceding_operations
                    .iter()
                    .filter_map(|preceding| match preceding {
                        TargetUnitOperation::EstablishAffineScalarRecord {
                            psi_operation,
                            result,
                            field,
                            value,
                            shape,
                        } if result.place == argument.place => {
                            Some((*psi_operation, result, *field, *value, *shape))
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                let exact_affine_scalar_record = matches!(
                    affine_scalar_record_source.as_slice(),
                    [(establishment, result, _, _, shape)]
                        if argument.path.is_empty()
                            && argument.access == terminal_psi::StructuralAccess::Owned
                            && argument.root_structural_type == result.structural_type
                            && argument.structural_type == result.structural_type
                            && argument.shape == ValueShape::integer(8, 8)
                            && *shape == argument.shape
                            && argument.source.shape == argument.shape
                            && argument.source.locations.is_empty()
                            && result.multiplicity == terminal_psi::StructuralMultiplicity::Affine
                            && result.qualifications.is_empty()
                            && result.projected_qualifications.is_empty()
                            && result.claims.is_empty()
                            && !preceding_operations.iter().any(|preceding| match preceding {
                                TargetUnitOperation::Call { arguments, .. }
                                | TargetUnitOperation::StructuralScalarCall { arguments, .. } => {
                                    arguments.iter().any(|candidate| {
                                        candidate.place == argument.place
                                            && candidate.path.is_empty()
                                            && candidate.access == terminal_psi::StructuralAccess::Owned
                                    })
                                }
                                _ => false,
                            })
                            && *establishment != *psi_operation
                );
                let exact_result_source = exact_structural_result_source(
                    body, preceding_operations, assigned_structural_homes, argument,
                );
                !parameter_source && !exact_trivial_local && !exact_affine_scalar_record && !exact_result_source
            }) {
                return Err(invalid());
            }
        AssignedUnitOperation::Call {
            psi_operation: *psi_operation,
            callee: *callee,
            result: None,
            call_plan: call_plan.clone(),
            transport: if assigned_scalar_arguments.is_empty() {
                None
            } else {
                Some(super::super::scalar_transport::assign(
                    call_plan,
                    &assigned_scalar_arguments,
                    target,
                    super::super::scalar_transport::CallTransportKind::Mixed,
                )?)
            },
            scalar_arguments: assigned_scalar_arguments,
            copies: arguments
                .iter()
                .map(|argument| AssignedAggregateCopy {
                    place: argument.place,
                    access: argument.access,
                    path: argument.path.clone(),
                    root_structural_type: argument.root_structural_type,
                    structural_type: argument.structural_type,
                    shape: argument.shape,
                    source_byte_offset: argument.source_byte_offset,
                    fixed_array_length: argument.fixed_array_length,
                    element_stride: argument.element_stride,
                    source: argument.source.clone(),
                    destination: argument.destination.clone(),
                })
                .collect(),
            claim_transfers: claim_transfers.clone(),
            requirement_obligations: requirement_obligations.clone(),
            crash_continuations: crash_continuations.clone(),
        }
    })
}

fn exact_structural_result_source(
    body: &target_operations::TargetUnitBody,
    preceding: &[TargetUnitOperation],
    homes: &BTreeMap<PlaceId, AssignedStructuralHome>,
    argument: &target_operations::TargetStructuralArgument,
) -> bool {
    let Some(home) = homes.get(&argument.place) else {
        return false;
    };
    let requirement = &home.requirement;
    let target_operations::TargetStructuralHomeLayout::Aggregate(shape) = requirement.layout else {
        return false;
    };
    let result = &requirement.result;
    result.place == argument.place
        && result.multiplicity == terminal_psi::StructuralMultiplicity::Affine
        && result.qualifications.is_empty()
        && result.projected_qualifications.is_empty()
        && result.claims.is_empty()
        && affine_projection::exact_owned_path(body, result.structural_type, shape, argument)
        && preceding
            .iter()
            .filter(|operation| {
                matches!(operation,
                    TargetUnitOperation::StructuralResultCall {
                        psi_operation, result: produced, result_home: Some(required), call_plan, ..
                    } if *psi_operation == requirement.defining_operation
                        && produced == result && required == requirement
                        && call_plan.result.as_ref() == Some(&argument.source)
                )
            })
            .count()
            == 1
        && !preceding.iter().any(|operation| match operation {
            TargetUnitOperation::Call { arguments, .. } => arguments.iter().any(|prior| {
                prior.place == argument.place
                    && prior.access == terminal_psi::StructuralAccess::Owned
                    && (prior.path.starts_with(&argument.path)
                        || argument.path.starts_with(&prior.path))
            }),
            _ => false,
        })
}
