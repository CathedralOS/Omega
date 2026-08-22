//! Canonical format-31 value-shape and placement codec.
//!
//! Owning rows retain their ordering and validation in the installation
//! parent. This child owns only the exact shape, register, and location bytes.

use omega_calling_conventions::{
    MachineRegister, ValueClass, ValueLocation, ValuePlacement, ValueShape,
};

use super::{Reader, TerminalInstallationError, decode_boolean, push_u16, push_u32};

pub(super) fn encode_shape(
    bytes: &mut Vec<u8>,
    shape: ValueShape,
) -> Result<(), TerminalInstallationError> {
    if shape.class != ValueClass::Integer {
        return Err(TerminalInstallationError::UnsupportedStructuralReturnShape);
    }
    bytes.push(1);
    bytes.push(0);
    push_u16(bytes, shape.byte_size);
    push_u16(bytes, shape.alignment);
    push_u16(bytes, 0);
    Ok(())
}

pub(super) fn encode_placement(
    bytes: &mut Vec<u8>,
    placement: &ValuePlacement,
) -> Result<(), TerminalInstallationError> {
    encode_shape(bytes, placement.shape)?;
    let [location] = placement.locations.as_slice() else {
        return Err(TerminalInstallationError::UnsupportedStructuralReturnPlacement);
    };
    match location {
        ValueLocation::Register {
            register,
            value_byte_offset,
            byte_size,
        } => {
            bytes.push(1);
            bytes.push(register_tag(*register)?);
            push_u16(bytes, *value_byte_offset);
            push_u16(bytes, *byte_size);
            push_u16(bytes, 0);
        }
        ValueLocation::Stack {
            stack_byte_offset,
            value_byte_offset,
            byte_size,
            alignment,
        } => {
            bytes.push(2);
            bytes.push(0);
            push_u16(bytes, *value_byte_offset);
            push_u16(bytes, *byte_size);
            push_u16(bytes, *alignment);
            push_u32(bytes, *stack_byte_offset);
        }
        ValueLocation::Indirect { .. } => {
            return Err(TerminalInstallationError::UnsupportedStructuralReturnPlacement);
        }
    }
    Ok(())
}

pub(super) fn encode_direct_placement(
    bytes: &mut Vec<u8>,
    placement: &ValuePlacement,
) -> Result<(), TerminalInstallationError> {
    encode_shape(bytes, placement.shape)?;
    push_u32(
        bytes,
        u32::try_from(placement.locations.len())
            .map_err(|_| TerminalInstallationError::UnsupportedInternalUnitCallPlacement)?,
    );
    for location in &placement.locations {
        match location {
            ValueLocation::Register {
                register,
                value_byte_offset,
                byte_size,
            } => {
                bytes.push(1);
                bytes.push(register_tag(*register)?);
                push_u16(bytes, *value_byte_offset);
                push_u16(bytes, *byte_size);
            }
            ValueLocation::Stack {
                stack_byte_offset,
                value_byte_offset,
                byte_size,
                alignment,
            } => {
                bytes.push(2);
                bytes.push(0);
                push_u16(bytes, *value_byte_offset);
                push_u16(bytes, *byte_size);
                push_u16(bytes, *alignment);
                push_u32(bytes, *stack_byte_offset);
            }
            ValueLocation::Indirect {
                pointer,
                copy_stack_byte_offset,
                byte_size,
                alignment,
            } => {
                bytes.push(3);
                match pointer {
                    omega_calling_conventions::IndirectPointerLocation::Register(register) => {
                        bytes.push(1);
                        bytes.push(register_tag(*register)?);
                        bytes.push(0);
                    }
                    omega_calling_conventions::IndirectPointerLocation::Stack {
                        stack_byte_offset,
                        alignment,
                    } => {
                        bytes.push(2);
                        bytes.push(0);
                        bytes.push(0);
                        push_u32(bytes, *stack_byte_offset);
                        push_u16(bytes, *alignment);
                    }
                }
                match copy_stack_byte_offset {
                    Some(offset) => {
                        bytes.push(1);
                        push_u32(bytes, *offset);
                    }
                    None => bytes.push(0),
                }
                push_u16(bytes, *byte_size);
                push_u16(bytes, *alignment);
            }
        }
    }
    Ok(())
}

pub(super) fn decode_shape(
    reader: &mut Reader<'_>,
) -> Result<ValueShape, TerminalInstallationError> {
    if reader.u8()? != 1 || reader.u8()? != 0 {
        return Err(TerminalInstallationError::UnsupportedStructuralReturnShape);
    }
    let byte_size = reader.u16()?;
    let alignment = reader.u16()?;
    if reader.u16()? != 0 {
        return Err(TerminalInstallationError::NonzeroReservedField);
    }
    Ok(ValueShape::integer(byte_size, alignment))
}

pub(super) fn decode_placement(
    reader: &mut Reader<'_>,
) -> Result<ValuePlacement, TerminalInstallationError> {
    let shape = decode_shape(reader)?;
    let location_kind = reader.u8()?;
    let detail = reader.u8()?;
    let location = match location_kind {
        1 => {
            let value_byte_offset = reader.u16()?;
            let byte_size = reader.u16()?;
            if reader.u16()? != 0 {
                return Err(TerminalInstallationError::NonzeroReservedField);
            }
            ValueLocation::Register {
                register: decode_register(detail)?,
                value_byte_offset,
                byte_size,
            }
        }
        2 => {
            if detail != 0 {
                return Err(TerminalInstallationError::NonzeroReservedField);
            }
            ValueLocation::Stack {
                value_byte_offset: reader.u16()?,
                byte_size: reader.u16()?,
                alignment: reader.u16()?,
                stack_byte_offset: reader.u32()?,
            }
        }
        _ => return Err(TerminalInstallationError::UnsupportedStructuralReturnPlacement),
    };
    Ok(ValuePlacement {
        shape,
        locations: vec![location],
    })
}

pub(super) fn decode_direct_placement(
    reader: &mut Reader<'_>,
) -> Result<ValuePlacement, TerminalInstallationError> {
    let shape = decode_shape(reader)?;
    let count = usize::try_from(reader.u32()?)
        .map_err(|_| TerminalInstallationError::UnsupportedInternalUnitCallPlacement)?;
    if count == 0 && shape.byte_size == 0 {
        return Ok(ValuePlacement {
            shape,
            locations: Vec::new(),
        });
    }
    if count == 0 || count > reader.remaining() / 6 {
        return Err(TerminalInstallationError::UnsupportedInternalUnitCallPlacement);
    }
    let mut locations = Vec::with_capacity(count);
    for _ in 0..count {
        locations.push(match reader.u8()? {
            1 => ValueLocation::Register {
                register: decode_register(reader.u8()?)?,
                value_byte_offset: reader.u16()?,
                byte_size: reader.u16()?,
            },
            2 => {
                if reader.u8()? != 0 {
                    return Err(TerminalInstallationError::NonzeroReservedField);
                }
                ValueLocation::Stack {
                    value_byte_offset: reader.u16()?,
                    byte_size: reader.u16()?,
                    alignment: reader.u16()?,
                    stack_byte_offset: reader.u32()?,
                }
            }
            3 => {
                let pointer = match reader.u8()? {
                    1 => {
                        let register = decode_register(reader.u8()?)?;
                        if reader.u8()? != 0 {
                            return Err(TerminalInstallationError::NonzeroReservedField);
                        }
                        omega_calling_conventions::IndirectPointerLocation::Register(register)
                    }
                    2 => {
                        if reader.take(2)? != [0; 2] {
                            return Err(TerminalInstallationError::NonzeroReservedField);
                        }
                        omega_calling_conventions::IndirectPointerLocation::Stack {
                            stack_byte_offset: reader.u32()?,
                            alignment: reader.u16()?,
                        }
                    }
                    _ => {
                        return Err(
                            TerminalInstallationError::UnsupportedInternalUnitCallPlacement,
                        );
                    }
                };
                let copy_stack_byte_offset = match decode_boolean(reader.u8()?)? {
                    true => Some(reader.u32()?),
                    false => None,
                };
                ValueLocation::Indirect {
                    pointer,
                    copy_stack_byte_offset,
                    byte_size: reader.u16()?,
                    alignment: reader.u16()?,
                }
            }
            _ => return Err(TerminalInstallationError::UnsupportedInternalUnitCallPlacement),
        });
    }
    Ok(ValuePlacement { shape, locations })
}

fn register_tag(register: MachineRegister) -> Result<u8, TerminalInstallationError> {
    match register {
        MachineRegister::X86Rax => Ok(1),
        MachineRegister::X86Rcx => Ok(2),
        MachineRegister::X86Rdi => Ok(3),
        MachineRegister::Aarch64X(0) => Ok(4),
        MachineRegister::X86Rsi => Ok(5),
        MachineRegister::X86Rdx => Ok(6),
        MachineRegister::Aarch64X(1) => Ok(7),
        MachineRegister::X86R8 => Ok(8),
        MachineRegister::X86R9 => Ok(9),
        MachineRegister::Aarch64X(2) => Ok(10),
        MachineRegister::Aarch64X(3) => Ok(11),
        MachineRegister::Aarch64X(4) => Ok(12),
        MachineRegister::Aarch64X(5) => Ok(13),
        MachineRegister::Aarch64X(6) => Ok(14),
        MachineRegister::Aarch64X(7) => Ok(15),
        _ => Err(TerminalInstallationError::UnsupportedStructuralReturnRegister(register)),
    }
}

fn decode_register(value: u8) -> Result<MachineRegister, TerminalInstallationError> {
    match value {
        1 => Ok(MachineRegister::X86Rax),
        2 => Ok(MachineRegister::X86Rcx),
        3 => Ok(MachineRegister::X86Rdi),
        4 => Ok(MachineRegister::Aarch64X(0)),
        5 => Ok(MachineRegister::X86Rsi),
        6 => Ok(MachineRegister::X86Rdx),
        7 => Ok(MachineRegister::Aarch64X(1)),
        8 => Ok(MachineRegister::X86R8),
        9 => Ok(MachineRegister::X86R9),
        10 => Ok(MachineRegister::Aarch64X(2)),
        11 => Ok(MachineRegister::Aarch64X(3)),
        12 => Ok(MachineRegister::Aarch64X(4)),
        13 => Ok(MachineRegister::Aarch64X(5)),
        14 => Ok(MachineRegister::Aarch64X(6)),
        15 => Ok(MachineRegister::Aarch64X(7)),
        _ => Err(TerminalInstallationError::InvalidStructuralReturnRegister(
            value,
        )),
    }
}
