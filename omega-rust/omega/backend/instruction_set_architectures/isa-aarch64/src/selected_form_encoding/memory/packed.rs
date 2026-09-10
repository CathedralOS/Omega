//! Exact little-endian fragments without native access widths.
//!
//! Scratch is a supplied early-clobber definition, never a hidden fixed
//! register. Each byte is accessed once; register padding is zero on loads
//! and never becomes a memory access. Replay decodes every instruction field.

use super::*;
use selected_instructions::PackedByteWidth;

fn request(
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    displacement: u32,
) -> Result<(bool, u8, [u8; 3]), Aarch64SelectedFormEncodingError> {
    let (load, byte_offset, width, family) = match kind {
        SelectedInstructionKind::LoadPacked { byte_offset, width } => (
            true,
            byte_offset,
            width,
            match width {
                PackedByteWidth::Three => MachineAlternativeFamily::LoadPacked3,
                PackedByteWidth::Five => MachineAlternativeFamily::LoadPacked5,
                PackedByteWidth::Six => MachineAlternativeFamily::LoadPacked6,
                PackedByteWidth::Seven => MachineAlternativeFamily::LoadPacked7,
            },
        ),
        SelectedInstructionKind::StorePacked { byte_offset, width } => (
            false,
            byte_offset,
            width,
            MachineAlternativeFamily::StorePacked,
        ),
        _ => return Err(Aarch64SelectedFormEncodingError::AlternativeMismatch),
    };
    let width = width.byte_size();
    if physical.model() != &aarch64_physical_register_model()
        || alternative != (MachineAlternativeKey { family, variant: 0 })
        || byte_offset != displacement
        || displacement
            .checked_add(u32::from(width) - 1)
            .is_none_or(|last| last > 4095)
    {
        return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
    }
    let registers: [u8; 3] = resolve_registers(physical, operands)?
        .try_into()
        .map_err(|_| Aarch64SelectedFormEncodingError::EncodedFormMismatch)?;
    let [base, value, scratch] = registers;
    if scratch == base || scratch == value || (load && base == value) {
        return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
    }
    Ok((load, width, registers))
}

pub(super) fn encode(
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    displacement: u32,
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    let (load, width, [base, value, scratch]) =
        request(physical, kind, alternative, operands, displacement)?;
    let (base, value, scratch) = (u32::from(base), u32::from(value), u32::from(scratch));
    let mut bytes = Vec::with_capacity(usize::from(width) * 8);
    if !load {
        bytes.extend_from_slice(&(0xaa00_03e0 | (value << 16) | scratch).to_le_bytes());
    }
    for byte in 0..u32::from(width) {
        let offset = displacement + byte;
        if load {
            let destination = if byte == 0 { value } else { scratch };
            bytes.extend_from_slice(
                &(0x3940_0000 | (offset << 10) | (base << 5) | destination).to_le_bytes(),
            );
            if byte != 0 {
                bytes.extend_from_slice(
                    &(0xaa00_0000 | (scratch << 16) | ((byte * 8) << 10) | (value << 5) | value)
                        .to_le_bytes(),
                );
            }
        } else {
            bytes.extend_from_slice(
                &(0x3900_0000 | (offset << 10) | (base << 5) | scratch).to_le_bytes(),
            );
            if byte + 1 != u32::from(width) {
                bytes.extend_from_slice(&(0xd348_fc00 | (scratch << 5) | scratch).to_le_bytes());
            }
        }
    }
    validate(physical, kind, alternative, operands, displacement, &bytes)
}

pub(super) fn validate(
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    displacement: u32,
    bytes: &[u8],
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    let (load, width, [base, value, scratch]) =
        request(physical, kind, alternative, operands, displacement)?;
    let instruction_count = usize::from(width) * 2 - usize::from(load);
    if bytes.len() != instruction_count * 4 {
        return Err(Aarch64SelectedFormEncodingError::MalformedEncoding);
    }
    let (base, value, scratch) = (u32::from(base), u32::from(value), u32::from(scratch));
    let mut words = bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|bytes| u32::from_le_bytes(*bytes));
    let invalid = || Aarch64SelectedFormEncodingError::EncodedFormMismatch;
    if !load {
        let copy = words.next().ok_or_else(invalid)?;
        if copy & !((31 << 16) | 31) != 0xaa00_03e0
            || (copy >> 16) & 31 != value
            || copy & 31 != scratch
        {
            return Err(invalid());
        }
    }
    for byte in 0..u32::from(width) {
        let memory = words.next().ok_or_else(invalid)?;
        let register = if load && byte == 0 { value } else { scratch };
        if memory & 0xffc0_0000 != if load { 0x3940_0000 } else { 0x3900_0000 }
            || memory & 31 != register
            || (memory >> 5) & 31 != base
            || (memory >> 10) & 4095 != displacement + byte
        {
            return Err(invalid());
        }
        if load && byte != 0 {
            let combine = words.next().ok_or_else(invalid)?;
            if combine & 0xffe0_0000 != 0xaa00_0000
                || (combine >> 16) & 31 != scratch
                || (combine >> 10) & 63 != byte * 8
                || (combine >> 5) & 31 != value
                || combine & 31 != value
            {
                return Err(invalid());
            }
        } else if !load && byte + 1 != u32::from(width) {
            let shift = words.next().ok_or_else(invalid)?;
            if shift & !((31 << 5) | 31) != 0xd348_fc00
                || (shift >> 5) & 31 != scratch
                || shift & 31 != scratch
            {
                return Err(invalid());
            }
        }
    }
    if words.next().is_some() {
        return Err(invalid());
    }
    Ok(ValidatedAarch64SelectedFormEncoding {
        bytes: bytes.to_vec(),
        footprint: Aarch64SelectedFormFootprint {
            register_reads: if load {
                vec![operands[0]]
            } else {
                vec![operands[0], operands[1]]
            },
            register_writes: if load {
                vec![operands[1], operands[2]]
            } else {
                vec![operands[2]]
            },
            writes_nzcv: false,
            encoded: MachineEncodedEffects {
                external_operand_reads: if load { vec![0] } else { vec![0, 1] },
                external_operand_writes: if load { vec![1, 2] } else { vec![2] },
                implicit_unit_uses: Vec::new(),
                implicit_unit_defs: Vec::new(),
                implicit_unit_clobbers: Vec::new(),
                memory: if load {
                    MachineEncodedMemoryEffect::ReadPointerV1 {
                        pointer_operand: 0,
                        byte_count: u16::from(width),
                    }
                } else {
                    MachineEncodedMemoryEffect::WritePointerV1 { pointer_operand: 0 }
                },
                stack: MachineEncodedStackEffect::UnchangedV1,
                trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
                control: MachineEncodedControlEffect::FallThroughV1,
            },
        },
    })
}

#[cfg(test)]
mod tests;
