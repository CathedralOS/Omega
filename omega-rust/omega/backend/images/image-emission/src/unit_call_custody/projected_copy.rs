//! Independent byte replay for owned projected values and their ABI copy slots.

use calling_conventions::{IndirectPointerLocation, ValueClass, ValueLocation};
use machine_code::InternalUnitCallArgumentRecord;
use target::{Architecture, NativeTarget};

use super::{
    aarch64_terminal_register, expected_aarch64_stack_load, expected_x86_stack_load,
    x86_terminal_register,
};

pub(super) fn expected_owned_projected_copy_bytes(
    target: NativeTarget,
    argument: &InternalUnitCallArgumentRecord,
) -> Option<Vec<u8>> {
    if argument.access != terminal_psi::StructuralAccess::Owned
        || argument.path.is_empty()
        || argument.shape.class != ValueClass::Integer
        || argument.source.shape.class != ValueClass::Integer
        || argument.destination.shape != argument.shape
        || argument.shape.byte_size == 0
        || argument.shape.alignment == 0
        || argument
            .source_byte_offset
            .checked_add(u32::from(argument.shape.byte_size))?
            > u32::from(argument.source.shape.byte_size)
    {
        return None;
    }
    let home = argument
        .call_stack_bytes
        .checked_add(argument.source_location.stack_byte_offset()?)?;
    let indirect_source = matches!(
        argument.source.locations.as_slice(),
        [ValueLocation::Indirect { .. }]
    );
    let mut bytes = Vec::new();
    if indirect_source {
        match target.architecture {
            Architecture::X86_64 => expected_x86_stack_load(&mut bytes, 11, home, 8)?,
            Architecture::Aarch64 => {
                bytes.extend_from_slice(&expected_aarch64_stack_load(9, home, 8)?.to_le_bytes())
            }
        }
    }
    if let [
        ValueLocation::Indirect {
            pointer,
            copy_stack_byte_offset: Some(temporary),
            byte_size,
            alignment,
        },
    ] = argument.destination.locations.as_slice()
    {
        if *byte_size != argument.shape.byte_size
            || *alignment != argument.shape.alignment
            || !temporary.is_multiple_of(u32::from(*alignment))
            || temporary.checked_add(u32::from(*byte_size))? > argument.call_stack_bytes
        {
            return None;
        }
        let mut copied = 0_u16;
        while copied < *byte_size {
            let remaining = byte_size.checked_sub(copied)?;
            let width = [8, 4, 2, 1].into_iter().find(|width| *width <= remaining)?;
            append_fragment(
                target,
                argument,
                &mut bytes,
                ValueLocation::Stack {
                    stack_byte_offset: temporary.checked_add(u32::from(copied))?,
                    value_byte_offset: copied,
                    byte_size: width,
                    alignment: width,
                },
            )?;
            copied = copied.checked_add(width)?;
        }
        let pointer_stack = match pointer {
            IndirectPointerLocation::Register(_) => None,
            IndirectPointerLocation::Stack {
                stack_byte_offset, ..
            } => {
                let end = stack_byte_offset.checked_add(8)?;
                if end > argument.call_stack_bytes
                    || (*stack_byte_offset < temporary.checked_add(u32::from(*byte_size))?
                        && *temporary < end)
                {
                    return None;
                }
                Some(*stack_byte_offset)
            }
        };
        match target.architecture {
            Architecture::X86_64 => {
                let register = match pointer {
                    IndirectPointerLocation::Register(register) => {
                        x86_terminal_register(*register)?
                    }
                    IndirectPointerLocation::Stack { .. } => 11,
                };
                bytes.extend_from_slice(&[
                    0x48 | (((register >> 3) & 1) << 2),
                    0x8d,
                    0x84 | ((register & 7) << 3),
                    0x24,
                ]);
                bytes.extend_from_slice(&temporary.to_le_bytes());
                if let Some(offset) = pointer_stack {
                    x86_stack_store(&mut bytes, register, offset, 8)?;
                }
            }
            Architecture::Aarch64 => {
                if *temporary > 0xfff {
                    return None;
                }
                let register = match pointer {
                    IndirectPointerLocation::Register(register) => {
                        aarch64_terminal_register(*register)?
                    }
                    IndirectPointerLocation::Stack { .. } => 10,
                };
                bytes.extend_from_slice(
                    &(0x9100_03e0 | (*temporary << 10) | u32::from(register)).to_le_bytes(),
                );
                if let Some(offset) = pointer_stack {
                    bytes.extend_from_slice(
                        &aarch64_stack_store(register, offset, 8)?.to_le_bytes(),
                    );
                }
            }
        }
    } else {
        let mut copied = 0_u16;
        for destination in &argument.destination.locations {
            let (offset, width) = match destination {
                ValueLocation::Register {
                    value_byte_offset,
                    byte_size,
                    ..
                }
                | ValueLocation::Stack {
                    value_byte_offset,
                    byte_size,
                    ..
                } => (*value_byte_offset, *byte_size),
                ValueLocation::Indirect { .. } => return None,
            };
            if offset != copied || !matches!(width, 1 | 2 | 4 | 8) {
                return None;
            }
            copied = copied.checked_add(width)?;
            if copied > argument.shape.byte_size {
                return None;
            }
            append_fragment(target, argument, &mut bytes, *destination)?;
        }
        if copied != argument.shape.byte_size {
            return None;
        }
    }
    Some(bytes)
}

fn append_fragment(
    target: NativeTarget,
    argument: &InternalUnitCallArgumentRecord,
    bytes: &mut Vec<u8>,
    destination: ValueLocation,
) -> Option<()> {
    let (value_offset, width, stack) = match destination {
        ValueLocation::Register {
            value_byte_offset,
            byte_size,
            ..
        } => (value_byte_offset, byte_size, None),
        ValueLocation::Stack {
            stack_byte_offset,
            value_byte_offset,
            byte_size,
            ..
        } => {
            if stack_byte_offset.checked_add(u32::from(byte_size))? > argument.call_stack_bytes {
                return None;
            }
            (value_byte_offset, byte_size, Some(stack_byte_offset))
        }
        ValueLocation::Indirect { .. } => return None,
    };
    let source_offset = argument
        .source_byte_offset
        .checked_add(u32::from(value_offset))?;
    let indirect = matches!(
        argument.source.locations.as_slice(),
        [ValueLocation::Indirect { .. }]
    );
    let offset = if indirect {
        source_offset
    } else {
        argument
            .call_stack_bytes
            .checked_add(argument.source_location.stack_byte_offset()?)?
            .checked_add(source_offset)?
    };
    match target.architecture {
        Architecture::X86_64 => {
            let register = match destination {
                ValueLocation::Register { register, .. } => x86_terminal_register(register)?,
                _ => 0,
            };
            if indirect {
                x86_pointer_load(bytes, register, offset, width)?;
            } else {
                expected_x86_stack_load(bytes, register, offset, width)?;
            }
            if let Some(offset) = stack {
                x86_stack_store(bytes, register, offset, width)?;
            }
        }
        Architecture::Aarch64 => {
            let register = match destination {
                ValueLocation::Register { register, .. } => aarch64_terminal_register(register)?,
                _ => 10,
            };
            let mut load = expected_aarch64_stack_load(register, offset, width)?;
            if indirect {
                load = (load & !(31 << 5)) | (9 << 5);
            }
            bytes.extend_from_slice(&load.to_le_bytes());
            if let Some(offset) = stack {
                bytes.extend_from_slice(
                    &aarch64_stack_store(register, offset, width)?.to_le_bytes(),
                );
            }
        }
    }
    Some(())
}

fn x86_pointer_load(bytes: &mut Vec<u8>, register: u8, offset: u32, width: u16) -> Option<()> {
    let prefix = 0x41 | (((register >> 3) & 1) << 2);
    match width {
        1 => bytes.extend_from_slice(&[prefix, 0x0f, 0xb6]),
        2 => bytes.extend_from_slice(&[0x66, prefix, 0x0f, 0xb7]),
        4 => bytes.extend_from_slice(&[prefix, 0x8b]),
        8 => bytes.extend_from_slice(&[prefix | 8, 0x8b]),
        _ => return None,
    }
    let register_bits = (register & 7) << 3;
    if offset == 0 {
        bytes.push(register_bits | 3);
    } else if offset <= i8::MAX as u32 {
        bytes.extend_from_slice(&[0x43 | register_bits, offset as u8]);
    } else {
        bytes.push(0x83 | register_bits);
        bytes.extend_from_slice(&offset.to_le_bytes());
    }
    Some(())
}

pub(super) fn x86_stack_store(
    bytes: &mut Vec<u8>,
    register: u8,
    offset: u32,
    width: u16,
) -> Option<()> {
    let prefix = 0x40 | (((register >> 3) & 1) << 2);
    match width {
        1 | 4 => bytes.push(prefix),
        2 => bytes.extend_from_slice(&[0x66, prefix]),
        8 => bytes.push(prefix | 8),
        _ => return None,
    }
    bytes.push(if width == 1 { 0x88 } else { 0x89 });
    if offset <= i8::MAX as u32 {
        bytes.extend_from_slice(&[0x44 | ((register & 7) << 3), 0x24, offset as u8]);
    } else {
        bytes.extend_from_slice(&[0x84 | ((register & 7) << 3), 0x24]);
        bytes.extend_from_slice(&offset.to_le_bytes());
    }
    Some(())
}

pub(super) fn aarch64_stack_store(register: u8, offset: u32, width: u16) -> Option<u32> {
    // The scaled load and store encodings differ only in the load bit.
    expected_aarch64_stack_load(register, offset, width).map(|load| load & !0x0040_0000)
}
