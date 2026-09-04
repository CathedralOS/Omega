//! Ordinary Unit-call assignment and structural custody replay.

use super::*;

pub(super) fn assign(
    machine: MachineId,
    body: &omega_target_operations::TargetUnitBody,
    operation: &TargetUnitOperation,
    preceding_operations: &[TargetUnitOperation],
    assigned_scalar_homes: &mut BTreeMap<ValueId, AssignedUnitScalarHome>,
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
                            psi_core::ScalarType::Boolean => ValueShape::integer(1, 1),
                            psi_core::ScalarType::Integer(integer) => {
                                super::scalar_call::fixed_integer_shape(source_value, integer)
                                    .map_err(|_| invalid())?
                            }
                            psi_core::ScalarType::IeeeFloat(_) => return Err(invalid()),
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
                let parameter_source = body.parameters.iter().any(|parameter| {
                    parameter.place == argument.place
                        && parameter.structural_type == argument.root_structural_type
                        && parameter.shape == argument.source.shape
                        && parameter.placement == argument.source
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
                        && argument.access == psi_terminal::StructuralAccess::Owned
                        && argument.root_structural_type == structural_type.id
                        && argument.structural_type == structural_type.id
                        && argument.shape == ValueShape::integer(0, 1)
                        && argument.source.shape == argument.shape
                        && argument.source.locations.is_empty()
                        && matches!(
                            place.kind,
                            psi_core::StructuralPlaceKind::TrivialAffineLocal {
                                structural_type: local_type,
                                construction: None,
                                ..
                            } if local_type == structural_type.id
                        )
                        && matches!(
                            structural_type.shape,
                            psi_terminal::StructuralTypeShape::Record { ref fields }
                                if fields.is_empty()
                        )
                        && body.structural_types.iter().any(|candidate| candidate == *structural_type)
                        && !preceding_operations.iter().any(|preceding| match preceding {
                            TargetUnitOperation::Call { arguments, .. }
                            | TargetUnitOperation::StructuralScalarCall { arguments, .. } => {
                                arguments.iter().any(|candidate| {
                                    candidate.place == argument.place
                                        && candidate.path.is_empty()
                                        && candidate.access == psi_terminal::StructuralAccess::Owned
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
                            && argument.access == psi_terminal::StructuralAccess::Owned
                            && argument.root_structural_type == result.structural_type
                            && argument.structural_type == result.structural_type
                            && argument.shape == ValueShape::integer(8, 8)
                            && *shape == argument.shape
                            && argument.source.shape == argument.shape
                            && argument.source.locations.is_empty()
                            && result.multiplicity == psi_terminal::StructuralMultiplicity::Affine
                            && result.qualifications.is_empty()
                            && result.projected_qualifications.is_empty()
                            && result.claims.is_empty()
                            && !preceding_operations.iter().any(|preceding| match preceding {
                                TargetUnitOperation::Call { arguments, .. }
                                | TargetUnitOperation::StructuralScalarCall { arguments, .. } => {
                                    arguments.iter().any(|candidate| {
                                        candidate.place == argument.place
                                            && candidate.path.is_empty()
                                            && candidate.access == psi_terminal::StructuralAccess::Owned
                                    })
                                }
                                _ => false,
                            })
                            && *establishment != *psi_operation
                );
                !parameter_source && !exact_trivial_local && !exact_affine_scalar_record
            }) {
                return Err(invalid());
            }
        AssignedUnitOperation::Call {
            psi_operation: *psi_operation,
            callee: *callee,
            result: None,
            call_plan: call_plan.clone(),
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
