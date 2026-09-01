use crate::assignment::shared::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn assign(
    machine: MachineId,
    psi_operation: OperationId,
    callee: MachineId,
    call_plan: &omega_calling_conventions::CallPlan,
    result: omega_target_operations::TargetUnitScalarHomeRequirement,
    arguments: &[omega_target_operations::TargetUnitScalarCallArgument],
    requirement_obligations: &[psi_core::ObligationId],
    crash_continuations: &[psi_terminal::CrashRouteBucket],
    preceding_operations: &[TargetUnitOperation],
    target: NativeTarget,
    assigned_homes: &mut BTreeMap<ValueId, AssignedUnitScalarHome>,
    next_home: &mut u32,
) -> Result<AssignedUnitOperation, AssignmentError> {
    let result_shape = fixed_integer_shape(result.source_value, result.scalar_type)?;
    if result.shape != result_shape || result.defining_operation != psi_operation {
        return Err(AssignmentError::UnitScalarCallCustodyMismatch {
            machine,
            operation: psi_operation,
        });
    }
    let expected_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: arguments
                .iter()
                .map(|argument| argument.placement.shape)
                .collect(),
            result: Some(result_shape),
        },
    )
    .map_err(|_| AssignmentError::UnitScalarCallCustodyMismatch {
        machine,
        operation: psi_operation,
    })?;
    if &expected_plan != call_plan
        || call_plan.parameters.len() != arguments.len()
        || call_plan.result.as_ref().is_none_or(|placement| {
            placement.shape != result_shape
                || !matches!(
                    placement.locations.as_slice(),
                    [ValueLocation::Register {
                        value_byte_offset: 0,
                        byte_size,
                        ..
                    }] if u32::from(*byte_size) == u32::from(result_shape.byte_size)
                )
        })
    {
        return Err(AssignmentError::UnitScalarCallCustodyMismatch {
            machine,
            operation: psi_operation,
        });
    }
    validate_placement_registers(
        result.source_value,
        call_plan.result.as_ref().unwrap(),
        target,
    )?;

    let assigned_arguments = arguments
        .iter()
        .enumerate()
        .map(|(parameter_index, argument)| {
            if usize::try_from(argument.parameter_index) != Ok(parameter_index)
                || call_plan.parameters.get(parameter_index) != Some(&argument.placement)
            {
                return Err(AssignmentError::UnitScalarCallCustodyMismatch {
                    machine,
                    operation: psi_operation,
                });
            }
            validate_placement_registers(
                argument.source.source_value(),
                &argument.placement,
                target,
            )?;
            let expected_shape = fixed_integer_shape(
                argument.source.source_value(),
                argument.source.scalar_type(),
            )?;
            if argument.placement.shape != expected_shape {
                return Err(AssignmentError::UnitScalarCallSourceMismatch(
                    argument.source.source_value(),
                ));
            }
            let source = assign_known_unit_scalar_source(
                argument.source,
                preceding_operations,
                assigned_homes,
            )
            .ok_or(AssignmentError::UnitScalarCallSourceMismatch(
                argument.source.source_value(),
            ))?;
            Ok(AssignedUnitScalarCallArgument {
                parameter_index: argument.parameter_index,
                source,
                destination: assigned_unit_scalar_destination(
                    argument.source.source_value(),
                    &argument.placement,
                    target,
                )?,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    *next_home = align_unit_scalar_offset(*next_home, 8)?;
    let result_home = AssignedUnitScalarHome {
        defining_operation: result.defining_operation,
        source_value: result.source_value,
        scalar_type: result.scalar_type,
        shape: result.shape,
        byte_offset: *next_home,
    };
    *next_home = next_home
        .checked_add(8)
        .ok_or(AssignmentError::UnitScalarHomeNotEncodable(
            result.source_value,
        ))?;
    if assigned_homes
        .insert(result.source_value, result_home)
        .is_some()
    {
        return Err(AssignmentError::UnitScalarCallSourceMismatch(
            result.source_value,
        ));
    }

    Ok(AssignedUnitOperation::ScalarCall {
        psi_operation,
        callee,
        call_plan: call_plan.clone(),
        result_home,
        arguments: assigned_arguments,
        requirement_obligations: requirement_obligations.to_vec(),
        crash_continuations: crash_continuations.to_vec(),
    })
}

/// Resolve one exact Unit scalar source against the ordered target stream and
/// the durable homes assigned so far. Internal and normalized foreign calls
/// share this join so neither can acquire a weaker source-custody path.
pub(super) fn assign_known_unit_scalar_source(
    source: TargetUnitScalarArgumentSource,
    preceding_operations: &[TargetUnitOperation],
    assigned_homes: &BTreeMap<ValueId, AssignedUnitScalarHome>,
) -> Option<AssignedUnitScalarArgumentSource> {
    match source {
        TargetUnitScalarArgumentSource::IntegerImmediate {
            defining_operation,
            source_value,
            scalar_type,
            value,
        } => {
            let matches = preceding_operations
                .iter()
                .filter(|operation| {
                    matches!(
                        operation,
                        TargetUnitOperation::IntegerConstant {
                            psi_operation,
                            result,
                            scalar_type: retained_type,
                            value: retained_value,
                        } if *psi_operation == defining_operation
                            && *result == source_value
                            && *retained_type == scalar_type
                            && *retained_value == value
                    )
                })
                .count();
            (matches == 1 && psi_core::ScalarTerm::integer(scalar_type, value).is_ok()).then_some(
                AssignedUnitScalarArgumentSource::IntegerImmediate {
                    defining_operation,
                    source_value,
                    scalar_type,
                    value,
                },
            )
        }
        TargetUnitScalarArgumentSource::Home(home) => {
            let matches = preceding_operations
                .iter()
                .filter(|operation| {
                    matches!(
                        operation,
                        TargetUnitOperation::ScalarCall { result_home, .. }
                            if *result_home == home
                    )
                })
                .count();
            let assigned = assigned_homes.get(&home.source_value).copied()?;
            (matches == 1
                && assigned.defining_operation == home.defining_operation
                && assigned.scalar_type == home.scalar_type
                && assigned.shape == home.shape)
                .then_some(AssignedUnitScalarArgumentSource::Home(assigned))
        }
    }
}

pub(super) fn unit_scalar_home_start(
    body: &omega_target_operations::TargetUnitBody,
    target: NativeTarget,
) -> Result<u32, AssignmentError> {
    let mut cursor = 0_u32;
    for parameter in &body.parameters {
        let alignment = match target.architecture {
            Architecture::X86_64 => 8,
            Architecture::Aarch64 => u32::from(parameter.shape.alignment.clamp(8, 16)),
        };
        cursor = align_unit_scalar_offset(cursor, alignment)?;
        let indirect = matches!(
            parameter.placement.locations.as_slice(),
            [ValueLocation::Indirect { .. }]
        );
        if parameter
            .placement
            .locations
            .iter()
            .any(|location| matches!(location, ValueLocation::Indirect { .. }))
            && !indirect
        {
            return Err(AssignmentError::UnitScalarFrameNotEncodable);
        }
        cursor = cursor
            .checked_add(if indirect {
                8
            } else {
                u32::from(parameter.shape.byte_size)
            })
            .ok_or(AssignmentError::UnitScalarFrameNotEncodable)?;
    }
    Ok(cursor)
}

fn align_unit_scalar_offset(value: u32, alignment: u32) -> Result<u32, AssignmentError> {
    value
        .checked_add(alignment.saturating_sub(1))
        .map(|value| value / alignment * alignment)
        .ok_or(AssignmentError::UnitScalarFrameNotEncodable)
}

fn fixed_integer_shape(
    value: ValueId,
    scalar_type: IntegerType,
) -> Result<ValueShape, AssignmentError> {
    if scalar_type.carrier() != psi_core::IntegerCarrier::Fixed
        || !matches!(scalar_type.bits(), 8 | 16 | 32 | 64)
    {
        return Err(AssignmentError::UnitScalarCallSourceMismatch(value));
    }
    let bytes = scalar_type.bits().div_ceil(8);
    Ok(ValueShape::integer(bytes, bytes.next_power_of_two().min(8)))
}

fn validate_placement_registers(
    value: ValueId,
    placement: &omega_calling_conventions::ValuePlacement,
    target: NativeTarget,
) -> Result<(), AssignmentError> {
    for location in &placement.locations {
        if let ValueLocation::Register { register, .. } = location {
            crate::assignment::placement::require_register_architecture(
                value,
                *register,
                target.architecture,
            )?;
        }
    }
    Ok(())
}

fn assigned_unit_scalar_destination(
    value: ValueId,
    placement: &omega_calling_conventions::ValuePlacement,
    target: NativeTarget,
) -> Result<AssignedCallDestination, AssignmentError> {
    match placement.locations.as_slice() {
        [
            ValueLocation::Register {
                register,
                value_byte_offset: 0,
                byte_size,
            },
        ] if *byte_size == placement.shape.byte_size => {
            crate::assignment::placement::require_register_architecture(
                value,
                *register,
                target.architecture,
            )?;
            Ok(AssignedCallDestination::Register(*register))
        }
        [
            ValueLocation::Stack {
                stack_byte_offset,
                value_byte_offset: 0,
                byte_size,
                ..
            },
        ] if *byte_size == placement.shape.byte_size => {
            Ok(AssignedCallDestination::OutgoingStack {
                byte_offset: *stack_byte_offset,
            })
        }
        _ => Err(AssignmentError::UnitScalarCallSourceMismatch(value)),
    }
}
