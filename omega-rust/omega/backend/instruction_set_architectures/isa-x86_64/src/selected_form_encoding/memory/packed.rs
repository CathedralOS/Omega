//! Exact byte accesses with an explicit early-clobber scratch register.
use super::*;
mod replay;

type Request = (bool, u8, [u8; 3], X86_64SelectedFormFootprint);

fn request(
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    displacement: u32,
) -> Result<Request, X86_64SelectedFormEncodingError> {
    if physical.model() != &crate::x86_64_physical_register_model() {
        return Err(X86_64SelectedFormEncodingError::NonCanonicalPhysicalModel);
    }
    let invalid = X86_64SelectedFormEncodingError::EncodedFormMismatch;
    let (load, width, offset) = match kind {
        SelectedInstructionKind::LoadPacked { byte_offset, width } => {
            (true, width.byte_size(), byte_offset)
        }
        SelectedInstructionKind::StorePacked { byte_offset, width } => {
            (false, width.byte_size(), byte_offset)
        }
        _ => return Err(invalid),
    };
    let family = if load {
        match width {
            3 => MachineAlternativeFamily::LoadPacked3,
            5 => MachineAlternativeFamily::LoadPacked5,
            6 => MachineAlternativeFamily::LoadPacked6,
            7 => MachineAlternativeFamily::LoadPacked7,
            _ => return Err(invalid),
        }
    } else {
        MachineAlternativeFamily::StorePacked
    };
    if alternative != (MachineAlternativeKey { family, variant: 0 }) {
        return Err(X86_64SelectedFormEncodingError::AlternativeMismatch);
    }
    if offset != displacement
        || displacement
            .checked_add(u32::from(width) - 1)
            .is_none_or(|end| end > i32::MAX as u32)
    {
        return Err(invalid);
    }
    let registers: [u8; 3] = resolve_registers(physical, operands)?
        .try_into()
        .map_err(|_| X86_64SelectedFormEncodingError::OperandCountMismatch)?;
    if registers[2] == registers[0]
        || registers[2] == registers[1]
        || (load && registers[0] == registers[1])
    {
        return Err(invalid);
    }
    let reads = if load { vec![0] } else { vec![0, 1] };
    let writes = if load { vec![1, 2] } else { vec![2] };
    let flags = physical
        .model()
        .view_named("rflags")
        .ok_or(X86_64SelectedFormEncodingError::NonCanonicalPhysicalModel)?;
    let footprint = X86_64SelectedFormFootprint {
        register_reads: reads
            .iter()
            .map(|position| operands[*position as usize])
            .collect(),
        register_writes: writes
            .iter()
            .map(|position| operands[*position as usize])
            .collect(),
        writes_rflags: true,
        encoded: MachineEncodedEffects {
            external_operand_reads: reads,
            external_operand_writes: writes,
            implicit_unit_uses: Vec::new(),
            implicit_unit_defs: Vec::new(),
            implicit_unit_clobbers: flags.units.clone(),
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
    };
    Ok((load, width, registers, footprint))
}

pub(super) fn encode(
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    displacement: u32,
) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
    let (load, width, [base, value, scratch], _) =
        request(physical, kind, alternative, operands, displacement)?;
    let mut bytes = Vec::new();
    if !load {
        bytes.extend([rex(value, 0, scratch), 0x89, modrm(3, value, scratch)]);
    }
    for byte_index in 0..width {
        let register = if load && byte_index == 0 {
            value
        } else {
            scratch
        };
        bytes.push(rex(register, 0, base) & if load { 0xff } else { 0xf7 });
        if load {
            bytes.push(0x0f);
        }
        bytes.extend([if load { 0xb6 } else { 0x88 }, modrm(2, register, base)]);
        if base & 7 == 4 {
            bytes.push(0x24);
        }
        bytes.extend((displacement + u32::from(byte_index)).to_le_bytes());
        if load && byte_index != 0 {
            bytes.extend([
                rex(0, 0, scratch),
                0xc1,
                modrm(3, 4, scratch),
                byte_index * 8,
            ]);
            bytes.extend([rex(scratch, 0, value), 0x09, modrm(3, scratch, value)]);
        } else if !load && byte_index + 1 != width {
            bytes.extend([rex(0, 0, scratch), 0xc1, modrm(3, 5, scratch), 8]);
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
) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
    let (load, width, registers, footprint) =
        request(physical, kind, alternative, operands, displacement)?;
    replay::validate(bytes, load, width, registers, displacement)?;
    Ok(ValidatedX86_64SelectedFormEncoding {
        bytes: bytes.to_vec(),
        footprint,
    })
}

#[cfg(test)]
mod tests;
