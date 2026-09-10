//! Resultless calls copy scalar arguments and transport original borrowed pointers.
use super::*;
use legalized_operations::{LegalizedScalarArgument, LegalizedScalarInstruction};
use selected_instructions::SelectedCallContract;

pub(super) fn emit(
    function: usize,
    source: &LegalizedScalarFunction,
    operation: &LegalizedScalarInstruction,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    builder: &mut Builder<'_>,
) -> Result<(), SelectedInstructionError> {
    let invalid = || SelectedInstructionError::SourceCustodyMismatch;
    let LegalizedScalarInstructionKind::Call(call) = &operation.kind else {
        return Err(invalid());
    };
    if operation.result.is_some()
        || (call.result_placement.is_some() && call.structural_result.is_none())
    {
        return Err(invalid());
    }
    call.validate_source(&operation.ownership)
        .map_err(|_| invalid())?;
    let key = crate::selection::scalar_call_abi::unit_key(call, environment).ok_or_else(invalid)?;
    crate::selection::scalar_call_abi::validate(
        function,
        source,
        call,
        operation.operation,
        key,
        row(builder.catalog, key)?,
        environment,
    )?;
    let mut operands = Vec::new();
    for (argument_index, argument) in call.arguments.iter().enumerate() {
        match argument {
            LegalizedScalarArgument::Structural { semantic, target } => {
                if semantic.access == StructuralAccess::Owned {
                    for register in super::aggregate_argument::argument(
                        source, operation, semantic, target, builder,
                    )? {
                        operands.push((argument_index, register));
                    }
                    continue;
                }
                if let Some(pointer) = super::scalar_call::argument_pointer(
                    builder,
                    operation,
                    argument_index,
                    semantic,
                    target,
                )? {
                    operands.push((argument_index, pointer));
                }
            }
            LegalizedScalarArgument::Scalar {
                source: value,
                placement,
            } => {
                if super::scalar_stack::argument(
                    builder,
                    operation,
                    argument_index,
                    *value,
                    placement,
                )? {
                    continue;
                }
                let (_, input, site, scalar_type) = builder.resolve(*value).ok_or_else(invalid)?;
                if crate::selection::scalar_call_abi::scalar_shape(scalar_type)
                    != Some(placement.shape)
                {
                    return Err(invalid());
                }
                let output = if let Some((kind, key)) =
                    crate::selection::scalar_call_abi::outgoing_float_transfer(
                        scalar_type,
                        &builder.constraints.keys,
                    ) {
                    let output = builder.register(*value, site, scalar_type)?;
                    builder.registers[output.0 as usize].class = row(builder.catalog, key)?
                        .operands
                        .get(1)
                        .ok_or_else(invalid)?
                        .class;
                    builder.emit(
                        kind,
                        key,
                        &[input, output],
                        SelectedInstructionProvenance {
                            values: vec![*value],
                            ..Default::default()
                        },
                    )?;
                    output
                } else {
                    builder.copy(input, *value, site, scalar_type)?
                };
                operands.push((argument_index, output));
            }
        }
    }
    let order = crate::selection::scalar_call_abi::register_argument_order(call);
    let mut operands = order
        .iter()
        .flat_map(|index| {
            operands
                .iter()
                .filter(move |(argument, _)| argument == index)
                .map(|(_, register)| *register)
        })
        .collect::<Vec<_>>();
    let mut result_registers = Vec::new();
    if let Some(result) = &call.structural_result {
        for location in &call
            .result_placement
            .as_ref()
            .ok_or_else(invalid)?
            .locations
        {
            let ValueLocation::Register {
                value_byte_offset, ..
            } = location
            else {
                return Err(invalid());
            };
            let register = super::structural_case::register(
                builder,
                result.place,
                u32::from(*value_byte_offset),
                64,
                false,
            )?;
            operands.push(register);
            result_registers.push(register);
        }
    }
    builder.transport.calls.push(SelectedCallContract {
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
        if call.structural_result.is_some() {
            SelectedInstructionKind::CallAggregate {
                callee: call.callee,
            }
        } else {
            SelectedInstructionKind::CallUnit {
                callee: call.callee,
            }
        },
        key,
        &operands,
        SelectedInstructionProvenance {
            operations: vec![operation.operation],
            values: call
                .arguments
                .iter()
                .filter_map(|argument| argument.scalar_source())
                .collect(),
            obligations: call.requirement_obligations.clone(),
            fuel: operation.fuel.clone(),
            ..Default::default()
        },
    )?;
    if let Some((result, placement)) =
        crate::selection::aggregate_result_input::call_result(source, call)
    {
        use selected_instructions::{
            FrameStorageSlotId, LocalStorageSlotId, SelectedLocalStorageSlot,
            SelectedMemoryAccessRole,
        };
        let slot = LocalStorageSlotId::Structural {
            operation: operation.operation,
            place: result.place,
        };
        builder
            .transport
            .local_slots
            .push(SelectedLocalStorageSlot {
                id: slot,
                byte_size: u32::from(placement.shape.byte_size),
                alignment: placement.shape.alignment,
            });
        let block = source
            .blocks
            .iter()
            .find(|block| {
                block
                    .instructions
                    .iter()
                    .any(|row| row.operation == operation.operation)
            })
            .ok_or_else(invalid)?
            .id;
        let pointer = super::structural_case::register(builder, result.place, 0, 64, false)?;
        super::structural_case::memory(
            builder,
            block,
            result.place,
            0,
            u32::from(placement.shape.byte_size),
            SelectedMemoryAccessRole::AddressLocal { slot },
        )?;
        builder.emit(
            SelectedInstructionKind::FrameAddress {
                slot: FrameStorageSlotId::Local(slot),
                byte_offset: 0,
            },
            builder.constraints.keys.frame_address.ok_or_else(invalid)?,
            &[pointer],
            Default::default(),
        )?;
        for (location, value) in placement.locations.iter().zip(result_registers) {
            let ValueLocation::Register {
                value_byte_offset,
                byte_size,
                ..
            } = location
            else {
                return Err(invalid());
            };
            super::structural_case::memory(
                builder,
                block,
                result.place,
                u32::from(*value_byte_offset),
                u32::from(*byte_size),
                SelectedMemoryAccessRole::WritePlace,
            )?;
            builder.emit(
                SelectedInstructionKind::Store {
                    byte_offset: u32::from(*value_byte_offset),
                    byte_size: *byte_size as u8,
                },
                builder.constraints.keys.store.ok_or_else(invalid)?,
                &[pointer, value],
                Default::default(),
            )?;
        }
    }
    Ok(())
}
