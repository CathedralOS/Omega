use super::*;

pub(crate) fn decode_target(
    cursor: &mut Cursor<'_>,
) -> Result<NativeTarget, PreAllocationMachineEffectDecodeError> {
    let architecture = match cursor.byte()? {
        0 => Architecture::Aarch64,
        1 => Architecture::X86_64,
        _ => return Err(PreAllocationMachineEffectDecodeError::InvalidField),
    };
    let object_format = match cursor.byte()? {
        0 => ObjectFormat::Elf,
        1 => ObjectFormat::MachO,
        2 => ObjectFormat::Coff,
        _ => return Err(PreAllocationMachineEffectDecodeError::InvalidField),
    };
    let pointer_size = usize::try_from(cursor.u64()?)
        .map_err(|_| PreAllocationMachineEffectDecodeError::InvalidField)?;
    let pointer_alignment = usize::try_from(cursor.u64()?)
        .map_err(|_| PreAllocationMachineEffectDecodeError::InvalidField)?;
    Ok(NativeTarget {
        architecture,
        object_format,
        pointer_size,
        pointer_alignment,
    })
}

pub(super) fn decode_constraint_key(
    cursor: &mut Cursor<'_>,
) -> Result<RegisterConstraintKey, PreAllocationMachineEffectDecodeError> {
    let family = match cursor.byte()? {
        0 => RegisterConstraintFamily::Call,
        1 => RegisterConstraintFamily::Return,
        2 => RegisterConstraintFamily::SystemCall,
        3 => RegisterConstraintFamily::InlineAssembly,
        4 => RegisterConstraintFamily::Instruction,
        _ => return Err(PreAllocationMachineEffectDecodeError::InvalidField),
    };
    Ok(RegisterConstraintKey {
        family,
        variant: cursor.u32()?,
    })
}

pub(crate) fn decode_units(
    cursor: &mut Cursor<'_>,
) -> Result<Vec<RegisterUnitId>, PreAllocationMachineEffectDecodeError> {
    let count = cursor.length()?;
    let mut units = Vec::with_capacity(count.min(cursor.remaining()));
    for _ in 0..count {
        units.push(RegisterUnitId(cursor.u16()?));
    }
    Ok(units)
}

pub(super) fn decode_ids<T>(
    cursor: &mut Cursor<'_>,
    constructor: impl Fn(u64) -> Option<T>,
) -> Result<Vec<T>, PreAllocationMachineEffectDecodeError> {
    let count = cursor.length()?;
    let mut values = Vec::with_capacity(count.min(cursor.remaining()));
    for _ in 0..count {
        values.push(
            constructor(cursor.u64()?)
                .ok_or(PreAllocationMachineEffectDecodeError::InvalidField)?,
        );
    }
    Ok(values)
}

pub(super) fn decode_machine(
    cursor: &mut Cursor<'_>,
) -> Result<MachineId, PreAllocationMachineEffectDecodeError> {
    MachineId::new(cursor.u64()?).ok_or(PreAllocationMachineEffectDecodeError::InvalidField)
}

pub(super) fn decode_obligation(
    cursor: &mut Cursor<'_>,
) -> Result<ObligationId, PreAllocationMachineEffectDecodeError> {
    ObligationId::new(cursor.u64()?).ok_or(PreAllocationMachineEffectDecodeError::InvalidField)
}
