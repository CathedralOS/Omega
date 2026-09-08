//! Pointer stores and constant address offsets with independent instruction replay.
use super::*;

fn request(
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    displacement: u32,
) -> Result<(u32, u32, u8, u8), Aarch64SelectedFormEncodingError> {
    let (family, byte_offset, opcode, scale) = match kind {
        SelectedInstructionKind::Store {
            byte_offset,
            byte_size,
        } => {
            let opcode = match byte_size {
                1 => 0x3900_0000,
                2 => 0x7900_0000,
                4 => 0xb900_0000,
                8 => 0xf900_0000,
                _ => return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch),
            };
            (
                MachineAlternativeFamily::Store,
                byte_offset,
                opcode,
                u32::from(byte_size),
            )
        }
        SelectedInstructionKind::AddressOffset { byte_offset } => (
            MachineAlternativeFamily::AddressOffset,
            byte_offset,
            0x9100_0000,
            1,
        ),
        _ => return Err(Aarch64SelectedFormEncodingError::AlternativeMismatch),
    };
    if physical.model() != &aarch64_physical_register_model()
        || alternative != (MachineAlternativeKey { family, variant: 0 })
        || byte_offset != displacement
        || !displacement.is_multiple_of(scale)
        || displacement / scale > 4095
    {
        return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
    }
    let registers = resolve_registers(physical, operands)?;
    let [first, second] = registers.as_slice() else {
        return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
    };
    Ok((opcode, scale, *first, *second))
}

pub(super) fn encode(
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    displacement: u32,
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    let (opcode, scale, base, register) =
        request(physical, kind, alternative, operands, displacement)?;
    let word =
        opcode | ((displacement / scale) << 10) | (u32::from(base) << 5) | u32::from(register);
    validate(
        physical,
        kind,
        alternative,
        operands,
        displacement,
        &word.to_le_bytes(),
    )
}

pub(super) fn validate(
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    displacement: u32,
    bytes: &[u8],
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    let (opcode, scale, base, register) =
        request(physical, kind, alternative, operands, displacement)?;
    let word = bytes
        .try_into()
        .ok()
        .map(u32::from_le_bytes)
        .ok_or(Aarch64SelectedFormEncodingError::MalformedEncoding)?;
    if word & 0xffc0_0000 != opcode
        || word & 31 != u32::from(register)
        || (word >> 5) & 31 != u32::from(base)
        || ((word >> 10) & 4095) * scale != displacement
    {
        return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
    }
    let store = matches!(kind, SelectedInstructionKind::Store { .. });
    Ok(ValidatedAarch64SelectedFormEncoding {
        bytes: bytes.to_vec(),
        footprint: Aarch64SelectedFormFootprint {
            register_reads: if store {
                operands.to_vec()
            } else {
                vec![operands[0]]
            },
            register_writes: if store { Vec::new() } else { vec![operands[1]] },
            writes_nzcv: false,
            encoded: MachineEncodedEffects {
                external_operand_reads: if store { vec![0, 1] } else { vec![0] },
                external_operand_writes: if store { Vec::new() } else { vec![1] },
                implicit_unit_uses: Vec::new(),
                implicit_unit_defs: Vec::new(),
                implicit_unit_clobbers: Vec::new(),
                memory: if store {
                    MachineEncodedMemoryEffect::WritePointerV1 { pointer_operand: 0 }
                } else {
                    MachineEncodedMemoryEffect::NoneV1
                },
                stack: MachineEncodedStackEffect::UnchangedV1,
                trap: if store {
                    MachineEncodedTrapBehavior::MayArchitecturalFaultV1
                } else {
                    MachineEncodedTrapBehavior::NeverV1
                },
                control: MachineEncodedControlEffect::FallThroughV1,
            },
        },
    })
}
