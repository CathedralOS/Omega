//! Rejoin entry-register preservation to the actual Unit ABI and frame.

use calling_conventions::ValueLocation;
use machine_code::{
    ParameterFunctionAbiRecord, UnitEntryRegisterSpillRecord, UnitParameterHomeRecord,
    UnitScalarParameterLocationRecord,
};
use semantic_vocabulary::ValueId;
use target::{Architecture, NativeTarget};

pub(crate) fn storage_end(
    target: NativeTarget,
    parameters: &[UnitParameterHomeRecord],
    abi: Option<&ParameterFunctionAbiRecord>,
    has_continuations: bool,
) -> Option<u32> {
    let mut cursor =
        crate::unit_call_custody::result_home::parameter_storage_end(target, parameters, abi)?;
    let Some(abi) = abi else {
        return Some(cursor);
    };
    if !has_continuations {
        return abi.entry_register_spills.is_empty().then_some(cursor);
    }
    let mut spill_position = 0;
    let mut values = Vec::new();
    for (parameter_index, parameter) in abi.parameters.iter().enumerate() {
        if values.contains(&parameter.value) {
            return None;
        }
        values.push(parameter.value);
        match parameter.placement.locations.as_slice() {
            [
                ValueLocation::Register {
                    register,
                    value_byte_offset: 0,
                    byte_size,
                },
            ] if *byte_size == parameter.placement.shape.byte_size => {
                cursor = cursor.checked_next_multiple_of(8)?;
                let spill = abi.entry_register_spills.get(spill_position)?;
                if spill.source_value != parameter.value
                    || spill.parameter_index != parameter_index
                    || spill.register != *register
                    || spill.byte_offset != cursor
                {
                    return None;
                }
                cursor = cursor.checked_add(8)?;
                spill_position += 1;
            }
            [
                ValueLocation::Stack {
                    value_byte_offset: 0,
                    byte_size,
                    ..
                },
            ] if *byte_size == parameter.placement.shape.byte_size => {}
            _ => return None,
        }
    }
    (spill_position == abi.entry_register_spills.len()).then_some(cursor)
}

pub(crate) fn store_bytes(
    target: NativeTarget,
    spill: &UnitEntryRegisterSpillRecord,
) -> Option<Vec<u8>> {
    let mut bytes = Vec::new();
    match target.architecture {
        Architecture::X86_64 => super::expected_x86_stack_store(
            &mut bytes,
            super::x86_terminal_register(spill.register)?,
            spill.byte_offset,
        ),
        Architecture::Aarch64 => bytes.extend_from_slice(
            &super::expected_aarch64_stack_store(
                super::aarch64_terminal_register(spill.register)?,
                spill.byte_offset,
            )?
            .to_le_bytes(),
        ),
    }
    Some(bytes)
}

pub(crate) fn validate_shape(
    target: NativeTarget,
    parameters: &[UnitParameterHomeRecord],
    abi: Option<&ParameterFunctionAbiRecord>,
    has_continuations: bool,
    frame_bytes: u32,
) -> Option<u32> {
    let storage_end = storage_end(target, parameters, abi, has_continuations)?;
    if !has_continuations {
        return Some(storage_end);
    }
    if storage_end > frame_bytes {
        return None;
    }
    let mut code_cursor = match target.architecture {
        Architecture::X86_64 => {
            if frame_bytes == 0 {
                0
            } else {
                crate::unit_stack::x86_64_stack_adjustment(frame_bytes, false).len()
            }
        }
        Architecture::Aarch64 => 8,
    };
    if let Some(abi) = abi {
        for spill in &abi.entry_register_spills {
            let expected = store_bytes(target, spill)?;
            if spill.code_offset != code_cursor || spill.byte_count != expected.len() {
                return None;
            }
            code_cursor = code_cursor.checked_add(expected.len())?;
        }
    }
    Some(storage_end)
}

pub(crate) fn exact_prologue(
    target: NativeTarget,
    bytes: &[u8],
    abi: Option<&ParameterFunctionAbiRecord>,
    parameters: &[UnitParameterHomeRecord],
    frame_bytes: u32,
    first_operation_offset: usize,
) -> bool {
    let mut cursor = match target.architecture {
        Architecture::X86_64 => {
            if frame_bytes == 0 {
                0
            } else {
                crate::unit_stack::x86_64_stack_adjustment(frame_bytes, false).len()
            }
        }
        Architecture::Aarch64 => 8,
    };
    if let Some(abi) = abi {
        for spill in &abi.entry_register_spills {
            let Some(expected) = store_bytes(target, spill) else {
                return false;
            };
            let Some(end) = cursor.checked_add(expected.len()) else {
                return false;
            };
            if spill.code_offset != cursor
                || spill.byte_count != expected.len()
                || bytes.get(cursor..end) != Some(expected.as_slice())
            {
                return false;
            }
            cursor = end;
        }
    }
    let Some(staging) = crate::unit_call_custody::parameter_staging::expected_bytes(
        target,
        parameters,
        frame_bytes,
    ) else {
        return false;
    };
    let Some(end) = cursor.checked_add(staging.len()) else {
        return false;
    };
    end == first_operation_offset && bytes.get(cursor..end) == Some(staging.as_slice())
}

pub(crate) fn parameter_location(
    abi: &ParameterFunctionAbiRecord,
    parameter_index: usize,
    source_value: ValueId,
    consumer_code_offset: usize,
) -> Option<UnitScalarParameterLocationRecord> {
    let parameter = abi.parameters.get(parameter_index)?;
    if parameter.value != source_value {
        return None;
    }
    if let Some(spill) = abi
        .entry_register_spills
        .iter()
        .find(|spill| spill.parameter_index == parameter_index)
    {
        return (spill.source_value == source_value
            && spill.code_offset.checked_add(spill.byte_count)? <= consumer_code_offset)
            .then_some(UnitScalarParameterLocationRecord::FrameSpill {
                byte_offset: spill.byte_offset,
            });
    }
    match parameter.placement.locations.as_slice() {
        [
            ValueLocation::Register {
                register,
                value_byte_offset: 0,
                byte_size,
            },
        ] if *byte_size == parameter.placement.shape.byte_size => {
            Some(UnitScalarParameterLocationRecord::Register(*register))
        }
        [
            ValueLocation::Stack {
                stack_byte_offset,
                value_byte_offset: 0,
                byte_size,
                ..
            },
        ] if *byte_size == parameter.placement.shape.byte_size => {
            Some(UnitScalarParameterLocationRecord::IncomingStack {
                byte_offset: *stack_byte_offset,
            })
        }
        _ => None,
    }
}
