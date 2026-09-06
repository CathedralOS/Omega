//! Copy an owned projected subtree into its ABI-mandated indirect temporary.

use super::*;

fn destination(
    copy: &AssignedAggregateCopy,
    call_stack_bytes: u32,
) -> Result<(IndirectPointerLocation, u32), EmissionError> {
    let [
        ValueLocation::Indirect {
            pointer,
            copy_stack_byte_offset: Some(offset),
            byte_size,
            alignment,
        },
    ] = copy.destination.locations.as_slice()
    else {
        return Err(EmissionError::UnsupportedAggregatePlacement);
    };
    if copy.destination.shape != copy.shape
        || *byte_size != copy.shape.byte_size
        || *alignment != copy.shape.alignment
        || *alignment == 0
        || !offset.is_multiple_of(u32::from(*alignment))
        || offset
            .checked_add(u32::from(*byte_size))
            .is_none_or(|end| end > call_stack_bytes)
    {
        return Err(EmissionError::UnsupportedAggregatePlacement);
    }
    Ok((*pointer, *offset))
}

fn width(remaining: u32) -> u16 {
    if remaining >= 8 {
        8
    } else if remaining >= 4 {
        4
    } else if remaining >= 2 {
        2
    } else {
        1
    }
}

pub(super) fn x86(
    bytes: &mut Vec<u8>,
    copy: &AssignedAggregateCopy,
    home: &X86UnitStructuralHome,
    call_stack_bytes: u32,
) -> Result<(), EmissionError> {
    let (pointer, temporary) = destination(copy, call_stack_bytes)?;
    let source_home = call_stack_bytes
        .checked_add(home.byte_offset)
        .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
    if home.indirect {
        emit_x86_64_stack_load_width(bytes, 11, source_home, 8)?;
    }
    let mut copied = 0_u32;
    while copied < u32::from(copy.shape.byte_size) {
        let width = width(u32::from(copy.shape.byte_size) - copied);
        let offset = copy
            .source_byte_offset
            .checked_add(copied)
            .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
        if home.indirect {
            emit_x86_64_memory_load_width(bytes, 0, 11, offset, width)?;
        } else {
            emit_x86_64_stack_load_width(
                bytes,
                0,
                source_home
                    .checked_add(offset)
                    .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?,
                width,
            )?;
        }
        emit_x86_64_stack_store_width(
            bytes,
            0,
            temporary
                .checked_add(copied)
                .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?,
            width,
        )?;
        copied += u32::from(width);
    }
    let register = match pointer {
        IndirectPointerLocation::Register(register) => x86_unit_register(register)?,
        IndirectPointerLocation::Stack { .. } => 11,
    };
    // lea destination, [rsp + displacement32]
    bytes.extend_from_slice(&[
        0x48 | (((register >> 3) & 1) << 2),
        0x8d,
        0x84 | ((register & 7) << 3),
        0x24,
    ]);
    bytes.extend_from_slice(&temporary.to_le_bytes());
    if let IndirectPointerLocation::Stack {
        stack_byte_offset, ..
    } = pointer
    {
        emit_x86_64_stack_store_width(bytes, register, stack_byte_offset, 8)?;
    }
    Ok(())
}

pub(super) fn aarch64(
    instructions: &mut Vec<u32>,
    copy: &AssignedAggregateCopy,
    home: &Aarch64UnitStructuralHome,
    call_stack_bytes: u32,
) -> Result<(), EmissionError> {
    let (pointer, temporary) = destination(copy, call_stack_bytes)?;
    let source_home = call_stack_bytes
        .checked_add(home.byte_offset)
        .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
    if home.indirect {
        instructions.push(aarch64_unit_stack_access(0xf940_0000, 9, source_home, 8)?);
    }
    let mut copied = 0_u32;
    while copied < u32::from(copy.shape.byte_size) {
        let width = width(u32::from(copy.shape.byte_size) - copied);
        let offset = copy
            .source_byte_offset
            .checked_add(copied)
            .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
        instructions.push(if home.indirect {
            aarch64_unit_memory_access(aarch64_load_base(width)?, 10, 9, offset, width)?
        } else {
            aarch64_unit_stack_access(
                aarch64_load_base(width)?,
                10,
                source_home
                    .checked_add(offset)
                    .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?,
                width,
            )?
        });
        instructions.push(aarch64_unit_stack_access(
            aarch64_store_base(width)?,
            10,
            temporary
                .checked_add(copied)
                .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?,
            width,
        )?);
        copied += u32::from(width);
    }
    let register = match pointer {
        IndirectPointerLocation::Register(register) => aarch64_unit_register(register)?,
        IndirectPointerLocation::Stack { .. } => 10,
    };
    emit_aarch64_sp_address(instructions, register, temporary)?;
    if let IndirectPointerLocation::Stack {
        stack_byte_offset, ..
    } = pointer
    {
        instructions.push(aarch64_unit_stack_access(
            0xf900_0000,
            register,
            stack_byte_offset,
            8,
        )?);
    }
    Ok(())
}
