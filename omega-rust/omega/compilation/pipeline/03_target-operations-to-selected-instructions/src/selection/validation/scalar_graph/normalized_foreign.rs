//! Independent replay of evaluated normalized foreign calls: the same
//! operand transport, the same unique per-plan row, and the same roster
//! record are reconstructed and compared, never trusted from the proposal.
use super::{
    LegalizedScalarFunction, LegalizedScalarInstructionKind, SelectedInstructionId,
    SelectedInstructionKind, SelectedInstructionProvenance, VirtualRegisterOrigin,
};
use crate::SelectedInstructionError;
use crate::selection::validation::scalar_graph::Replay;
use calling_conventions::ValueLocation;
use legalized_operations::LegalizedScalarInstruction;
use selected_instructions::{
    LocalStorageSlotId, SelectedLocalStorageSlot, SelectedNormalizedForeignCall,
};

pub(super) fn validate(
    source: &LegalizedScalarFunction,
    operation: &LegalizedScalarInstruction,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    replay: &mut Replay<'_>,
) -> Result<(), SelectedInstructionError> {
    let LegalizedScalarInstructionKind::NormalizedForeignCall(call) = &operation.kind else {
        return Err(replay.invalid());
    };
    if operation.result.is_some() != call.result.scalar_home().is_some() {
        return Err(replay.invalid());
    }
    let constraint =
        crate::selection::scalar_call_abi::normalized_foreign::call_constraint(call, environment)
            .ok_or_else(|| replay.invalid())?;
    let key = constraint.key;
    if !replay
        .constraints
        .keys
        .call_normalized_foreign
        .contains(&key)
    {
        return Err(replay.invalid());
    }
    crate::selection::scalar_call_abi::normalized_foreign::validate(
        replay.function,
        source,
        operation,
        key,
        &constraint,
        environment,
    )?;
    let saved_controls = call.binding.import().map(|_| LocalStorageSlotId::Boundary {
        operation: operation.operation,
    });
    let mut operands = Vec::new();
    for argument in &call.scalar_arguments {
        let value = argument.source.source_value();
        if super::scalar_stack::argument(
            replay,
            operation,
            argument.parameter_index as usize,
            value,
            &argument.placement,
        )? {
            continue;
        }
        let (_, input, site, scalar_type) =
            replay.resolve(value).ok_or_else(|| replay.invalid())?;
        if scalar_type != argument.source.scalar_type()
            || crate::selection::scalar_call_abi::scalar_shape(scalar_type)
                != Some(argument.placement.shape)
        {
            return Err(replay.invalid());
        }
        let (kind, transfer_key) = crate::selection::scalar_call_abi::outgoing_float_transfer(
            scalar_type,
            &replay.constraints.keys,
        )
        .unwrap_or((
            SelectedInstructionKind::CopyI64,
            replay.constraints.keys.copy_i64,
        ));
        let class = environment
            .constraint(transfer_key)
            .and_then(|row| row.operands.get(1))
            .ok_or_else(|| replay.invalid())?
            .class;
        let output = replay.check_register_class(
            class,
            site,
            scalar_type,
            VirtualRegisterOrigin::InstructionResult {
                instruction: SelectedInstructionId(
                    replay
                        .instruction_cursor
                        .try_into()
                        .map_err(|_| replay.invalid())?,
                ),
                source_value: value,
            },
            None,
        )?;
        replay.check_instruction(
            kind,
            transfer_key,
            &[input, output],
            &SelectedInstructionProvenance {
                operations: vec![operation.operation],
                values: vec![value],
                ..Default::default()
            },
        )?;
        operands.push((argument.parameter_index, output));
    }
    for (argument_index, argument) in
        crate::selection::scalar_call_abi::normalized_foreign::structural_parameter_positions(call)
            .zip(&call.structural_arguments)
    {
        // The retained target argument has already rejoined its source type,
        // access and projection. Reuse ordinary value/reference transport,
        // without manufacturing an internal call or changing its ownership.
        if !argument.indices.is_empty() {
            return Err(SelectedInstructionError::custody());
        }
        let semantic = terminal_psi::StructuralArgument {
            place: argument.place,
            path: argument.path.clone(),
            access: argument.access,
        };
        operands.extend(
            super::call_arguments::structural_argument(
                source,
                operation,
                argument_index,
                &semantic,
                argument,
                &[],
                replay,
            )?
            .into_iter()
            .map(|register| (argument_index as u32, register)),
        );
    }
    // Preserve parameter order and each placement's fragment order, using
    // the same bank ordering as retained-plan constraint reconstruction.
    operands.sort_by_key(|(position, _)| {
        (
            call.binding.boundary_entry_plan.call.parameters[*position as usize]
                .shape
                .class
                == calling_conventions::ValueClass::Float,
            *position,
        )
    });
    let mut operands: Vec<_> = operands.into_iter().map(|(_, register)| register).collect();
    let hidden = super::structural::indirect_results::prepare_call(source, operation, replay)?;
    // Marshaling only moves bytes and addresses; it does not change floating
    // controls. Save immediately around the counterparty, after preparation,
    // and before result registers acquire the call instruction's origin.
    if let Some(saved_controls) = saved_controls {
        replay.transport.local_slots.push(SelectedLocalStorageSlot {
            id: saved_controls,
            byte_size: 8,
            alignment: 8,
        });
        replay.check_instruction(
            SelectedInstructionKind::SaveFloatingControl {
                slot: saved_controls,
            },
            replay
                .constraints
                .keys
                .save_floating_control
                .ok_or_else(|| replay.invalid())?,
            &[],
            &SelectedInstructionProvenance {
                operations: vec![operation.operation],
                ..Default::default()
            },
        )?;
    }
    let mut result_registers = Vec::new();
    if let Some(pointer) = hidden {
        operands.push(pointer);
    } else if let Some((result, placement, _)) =
        crate::selection::aggregate_result_input::call_completion(source, operation)
    {
        for location in &placement.locations {
            let ValueLocation::Register {
                value_byte_offset, ..
            } = location
            else {
                return Err(replay.invalid());
            };
            let class = constraint
                .operands
                .get(operands.len())
                .ok_or_else(|| replay.invalid())?
                .class;
            let expected_origin = VirtualRegisterOrigin::AbiTransport {
                instruction: SelectedInstructionId(
                    replay
                        .instruction_cursor
                        .try_into()
                        .map_err(|_| replay.invalid())?,
                ),
                place: result.place,
                byte_offset: *value_byte_offset,
            };
            let expected_type = semantic_vocabulary::ScalarType::Integer(
                semantic_vocabulary::IntegerType::new(
                    semantic_vocabulary::IntegerSign::Unsigned,
                    64,
                )
                .map_err(|_| replay.invalid())?,
            );
            let retained = replay
                .selected
                .virtual_registers
                .get(replay.register_cursor)
                .ok_or_else(|| replay.invalid())?;
            if retained.id.0 as usize != replay.register_cursor
                || retained.class != class
                || retained.origin != expected_origin
                || retained.scalar_type != expected_type
                || retained.definition_site.is_some()
                || retained.entry_fixed_view.is_some()
            {
                return Err(replay.invalid());
            }
            let register = retained.id;
            replay.register_cursor += 1;
            operands.push(register);
            result_registers.push(register);
        }
    }
    let short_result = if let Some(result) = operation.result {
        let class = constraint
            .operands
            .last()
            .ok_or_else(|| replay.invalid())?
            .class;
        let register = replay.check_register_class(
            class,
            result.definition_site,
            result.scalar_type,
            VirtualRegisterOrigin::InstructionResult {
                instruction: SelectedInstructionId(
                    replay
                        .instruction_cursor
                        .try_into()
                        .map_err(|_| replay.invalid())?,
                ),
                source_value: result.value,
            },
            None,
        )?;
        operands.push(register);
        Some(register)
    } else {
        None
    };
    let ordinal = replay.transport.normalized_foreign_calls.len();
    replay
        .transport
        .normalized_foreign_calls
        .push(SelectedNormalizedForeignCall {
            instruction: SelectedInstructionId(
                replay
                    .instruction_cursor
                    .try_into()
                    .map_err(|_| replay.invalid())?,
            ),
            operation: operation.operation,
            call: call.clone(),
            effect: operation.effect,
            ownership: operation.ownership.clone(),
        });
    replay.check_instruction(
        SelectedInstructionKind::NormalizedForeignCall {
            boundary: call.boundary,
            ordinal: ordinal.try_into().map_err(|_| replay.invalid())?,
        },
        key,
        &operands,
        &SelectedInstructionProvenance {
            operations: vec![operation.operation],
            values: call
                .scalar_arguments
                .iter()
                .map(|argument| argument.source.source_value())
                .chain(operation.result.map(|result| result.value))
                .collect(),
            fuel: operation.fuel.clone(),
            ..Default::default()
        },
    )?;
    if let Some(saved_controls) = saved_controls {
        replay.check_instruction(
            SelectedInstructionKind::RestoreFloatingControl {
                slot: saved_controls,
            },
            replay
                .constraints
                .keys
                .restore_floating_control
                .ok_or_else(|| replay.invalid())?,
            &[],
            &SelectedInstructionProvenance {
                operations: vec![operation.operation],
                ..Default::default()
            },
        )?;
    }
    if hidden.is_some() {
        super::structural::indirect_results::finish_call(source, operation, replay)?;
    } else {
        super::unit_call::retain_result(source, operation, &result_registers, replay)?;
    }
    if let (Some(short_result), Some(result)) = (short_result, operation.result) {
        let output =
            replay.result_register(result.value, result.definition_site, result.scalar_type)?;
        let (kind, transfer_key) = crate::selection::scalar_call_abi::incoming_float_transfer(
            result.scalar_type,
            &replay.constraints.keys,
        )
        .unwrap_or((
            crate::selection::scalar_call_abi::integer_carrier_normalization(result.scalar_type),
            replay.constraints.keys.copy_i64,
        ));
        replay.check_instruction(
            kind,
            transfer_key,
            &[short_result, output],
            &SelectedInstructionProvenance {
                operations: vec![operation.operation],
                values: vec![result.value],
                ..Default::default()
            },
        )?;
        replay.definitions.push((
            result.value,
            output,
            result.definition_site,
            result.scalar_type,
        ));
    }
    Ok(())
}
