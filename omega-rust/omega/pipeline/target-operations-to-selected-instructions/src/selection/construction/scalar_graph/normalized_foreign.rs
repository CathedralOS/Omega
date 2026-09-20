//! Evaluated normalized foreign calls: scalar copies, source-rooted borrowed
//! pointers, the exact per-plan register row, and the foreign-call roster.
//!
//! Every retained coordinate — locator target, calling policy, plan,
//! argument placements, and scalar result home — is re-validated by
//! `scalar_call_abi::normalized_foreign::validate` before any instruction is
//! emitted, so a substituted plan or argument cannot reach a selected row.
use super::{
    Builder, LegalizedScalarFunction, LegalizedScalarInstructionKind, SelectedInstructionId,
    SelectedInstructionKind, SelectedInstructionProvenance, VirtualRegisterId,
};
use crate::SelectedInstructionError;
use crate::selection::construction::scalar_graph::row;
use crate::selection::construction::scalar_graph::structural;
use calling_conventions::ValueLocation;
use legalized_operations::LegalizedScalarInstruction;
use selected_instructions::{SelectedMemoryAccessRole, SelectedNormalizedForeignCall};

pub(super) fn emit(
    function: usize,
    source: &LegalizedScalarFunction,
    operation: &LegalizedScalarInstruction,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    builder: &mut Builder<'_>,
) -> Result<(), SelectedInstructionError> {
    let invalid = || SelectedInstructionError::SourceCustodyMismatch;
    let LegalizedScalarInstructionKind::NormalizedForeignCall(call) = &operation.kind else {
        return Err(invalid());
    };
    if operation.result.is_some() != call.result_home.is_some() {
        return Err(invalid());
    }
    let key = crate::selection::scalar_call_abi::normalized_foreign::call_key(call, environment)
        .ok_or_else(invalid)?;
    if !builder
        .constraints
        .keys
        .call_normalized_foreign
        .contains(&key)
    {
        return Err(invalid());
    }
    crate::selection::scalar_call_abi::normalized_foreign::validate(
        function,
        source,
        operation,
        key,
        row(builder.catalog, key)?,
        environment,
    )?;
    // Scalar arguments copy their resolved source register; a stack-placed
    // argument is outgoing frame custody rather than a call operand.
    let mut operands = Vec::new();
    for argument in &call.scalar_arguments {
        let value = argument.source.source_value();
        if super::scalar_stack::argument(
            builder,
            operation,
            argument.parameter_index as usize,
            value,
            &argument.placement,
        )? {
            continue;
        }
        let (_, input, site, scalar_type) = builder.resolve(value).ok_or_else(invalid)?;
        if scalar_type != argument.source.scalar_type()
            || crate::selection::scalar_call_abi::scalar_shape(scalar_type)
                != Some(argument.placement.shape)
        {
            return Err(invalid());
        }
        let output = builder.copy(input, value, site, scalar_type)?;
        operands.push((argument.parameter_index, output));
    }
    // Each structural argument transports its source-rooted referent pointer
    // into the exact pointer word the plan assigned: a register-resident
    // destination is a call operand, a stack destination is an outgoing slot
    // store of that pointer.
    for (argument_index, argument) in
        crate::selection::scalar_call_abi::normalized_foreign::structural_parameter_positions(call)
            .zip(&call.structural_arguments)
    {
        let pointer = structural::place_pointer(
            builder,
            operation,
            argument.place,
            argument.source_byte_offset,
        )?;
        match argument.destination.locations.as_slice() {
            [ValueLocation::Register { .. }] => {
                operands.push((argument_index as u32, pointer));
            }
            [
                ValueLocation::Stack {
                    stack_byte_offset, ..
                },
            ] => {
                let slot = selected_instructions::OutgoingArgumentSlotId {
                    role: selected_instructions::OutgoingArgumentSlotRole::Argument,
                    operation: operation.operation,
                    argument_index: argument_index.try_into().map_err(|_| invalid())?,
                };
                builder
                    .transport
                    .slots
                    .push(selected_instructions::SelectedOutgoingArgumentSlot {
                        id: slot,
                        byte_size: 8,
                        alignment: 8,
                        abi_stack_byte_offset: *stack_byte_offset,
                    });
                builder
                    .transport
                    .memory
                    .push(selected_instructions::SelectedMemoryAccess {
                        instruction: SelectedInstructionId(
                            builder
                                .instructions
                                .len()
                                .try_into()
                                .map_err(|_| invalid())?,
                        ),
                        origin: selected_instructions::SelectedMemoryAccessOrigin::Operation(
                            operation.operation,
                        ),
                        place: argument.place,
                        byte_offset: 0,
                        byte_count: 8,
                        role: SelectedMemoryAccessRole::WriteOutgoing { slot },
                    });
                builder.emit(
                    SelectedInstructionKind::Store64 {
                        slot: selected_instructions::FrameStorageSlotId::Outgoing(slot),
                        byte_offset: 0,
                    },
                    builder.constraints.keys.store64.ok_or_else(invalid)?,
                    &[pointer],
                    SelectedInstructionProvenance {
                        operations: vec![operation.operation],
                        ..Default::default()
                    },
                )?;
            }
            _ => return Err(invalid()),
        }
    }
    operands.sort_by_key(|(position, _)| *position);
    let mut operands: Vec<VirtualRegisterId> =
        operands.into_iter().map(|(_, register)| register).collect();
    let short_result = if let Some(result) = operation.result {
        let register =
            builder.register(result.value, result.definition_site, result.scalar_type)?;
        builder.registers[register.0 as usize].class = row(builder.catalog, key)?
            .operands
            .last()
            .ok_or_else(invalid)?
            .class;
        operands.push(register);
        Some(register)
    } else {
        None
    };
    let ordinal = builder.transport.normalized_foreign_calls.len();
    builder
        .transport
        .normalized_foreign_calls
        .push(SelectedNormalizedForeignCall {
            instruction: SelectedInstructionId(
                builder
                    .instructions
                    .len()
                    .try_into()
                    .map_err(|_| invalid())?,
            ),
            operation: operation.operation,
            call: call.clone(),
            effect: operation.effect,
            ownership: operation.ownership.clone(),
        });
    builder.emit(
        SelectedInstructionKind::NormalizedForeignCall {
            boundary: call.boundary,
            ordinal: ordinal.try_into().map_err(|_| invalid())?,
        },
        key,
        &operands,
        SelectedInstructionProvenance {
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
    // The ABI result register carries the raw scalar carrier; the durable
    // definition is its signed/unsigned normalization, like internal calls.
    if let (Some(short_result), Some(result)) = (short_result, operation.result) {
        let output = builder.register(result.value, result.definition_site, result.scalar_type)?;
        builder.emit(
            crate::selection::scalar_call_abi::integer_carrier_normalization(result.scalar_type),
            builder.constraints.keys.copy_i64,
            &[short_result, output],
            SelectedInstructionProvenance {
                values: vec![result.value],
                ..Default::default()
            },
        )?;
        builder.definitions.push((
            result.value,
            output,
            result.definition_site,
            result.scalar_type,
        ));
    }
    Ok(())
}
