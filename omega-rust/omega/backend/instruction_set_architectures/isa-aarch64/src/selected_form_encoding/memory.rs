//! Unsigned-offset LDR with independently decoded pointer-read evidence.
use super::*;
mod indexed;

pub fn encode_aarch64_selected_memory_form(
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    displacement: u32,
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    if kind == SelectedInstructionKind::Load8Indexed {
        return indexed::encode(physical, alternative, operands, displacement);
    }
    let [base, destination] = request(physical, kind, alternative, operands, displacement)?;
    let word =
        0xf940_0000 | ((displacement / 8) << 10) | (u32::from(base) << 5) | u32::from(destination);
    validate_aarch64_selected_memory_form(
        physical,
        kind,
        alternative,
        operands,
        displacement,
        &word.to_le_bytes(),
    )
}

pub fn validate_aarch64_selected_memory_form(
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    displacement: u32,
    bytes: &[u8],
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    if kind == SelectedInstructionKind::Load8Indexed {
        return indexed::validate(physical, alternative, operands, displacement, bytes);
    }
    let [base, destination] = request(physical, kind, alternative, operands, displacement)?;
    let word = bytes
        .try_into()
        .ok()
        .map(u32::from_le_bytes)
        .filter(|word| word & 0xffc0_0000 == 0xf940_0000)
        .ok_or(Aarch64SelectedFormEncodingError::MalformedEncoding)?;
    if (word & 31) != u32::from(destination)
        || ((word >> 5) & 31) != u32::from(base)
        || ((word >> 10) & 4095) * 8 != displacement
    {
        return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
    }
    Ok(ValidatedAarch64SelectedFormEncoding {
        bytes: bytes.to_vec(),
        footprint: Aarch64SelectedFormFootprint {
            register_reads: vec![operands[0]],
            register_writes: vec![operands[1]],
            writes_nzcv: false,
            encoded: MachineEncodedEffects {
                external_operand_reads: vec![0],
                external_operand_writes: vec![1],
                implicit_unit_uses: Vec::new(),
                implicit_unit_defs: Vec::new(),
                implicit_unit_clobbers: Vec::new(),
                memory: MachineEncodedMemoryEffect::ReadPointerV1 {
                    pointer_operand: 0,
                    byte_count: 8,
                },
                stack: MachineEncodedStackEffect::UnchangedV1,
                trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
                control: MachineEncodedControlEffect::FallThroughV1,
            },
        },
    })
}

fn request(
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    displacement: u32,
) -> Result<[u8; 2], Aarch64SelectedFormEncodingError> {
    if physical.model() != &aarch64_physical_register_model()
        || !matches!(kind, SelectedInstructionKind::Load64 { byte_offset } if byte_offset == displacement)
        || alternative
            != (MachineAlternativeKey {
                family: MachineAlternativeFamily::Load64,
                variant: 0,
            })
        || !displacement.is_multiple_of(8)
        || displacement / 8 > 4095
    {
        return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
    }
    resolve_registers(physical, operands)?
        .try_into()
        .map_err(|_| Aarch64SelectedFormEncodingError::EncodedFormMismatch)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pointer_load_replays_opcode_registers_and_scaled_offset() {
        let physical =
            register_model::validate_physical_register_model(aarch64_physical_register_model())
                .unwrap();
        let operands = [
            physical.model().view_named("x1").unwrap().id,
            physical.model().view_named("x2").unwrap().id,
        ];
        let alternative = MachineAlternativeKey {
            family: MachineAlternativeFamily::Load64,
            variant: 0,
        };
        for displacement in [0, 8, 32760] {
            let kind = SelectedInstructionKind::Load64 {
                byte_offset: displacement,
            };
            let encoded = encode_aarch64_selected_memory_form(
                &physical,
                kind,
                alternative,
                &operands,
                displacement,
            )
            .unwrap();
            assert_eq!(encoded.bytes().len(), 4);
            for byte in 0..4 {
                let mut corrupt = encoded.bytes().to_vec();
                corrupt[byte] ^= 1;
                assert!(
                    validate_aarch64_selected_memory_form(
                        &physical,
                        kind,
                        alternative,
                        &operands,
                        displacement,
                        &corrupt
                    )
                    .is_err()
                );
            }
            assert!(
                validate_aarch64_selected_memory_form(
                    &physical,
                    kind,
                    alternative,
                    &[operands[1], operands[0]],
                    displacement,
                    encoded.bytes()
                )
                .is_err()
            );
        }
        for displacement in [1, 32768] {
            assert!(
                encode_aarch64_selected_memory_form(
                    &physical,
                    SelectedInstructionKind::Load64 {
                        byte_offset: displacement
                    },
                    alternative,
                    &operands,
                    displacement
                )
                .is_err()
            );
        }
    }
}
