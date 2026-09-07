//! LDRB Wt, [Xn, Xm], with full-width runtime indexing and a zero-extended result.
use super::*;

pub(super) fn encode(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    displacement: u32,
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    let [base, index, destination] = request(physical, alternative, operands, displacement)?;
    let word =
        0x3860_6800 | (u32::from(index) << 16) | (u32::from(base) << 5) | u32::from(destination);
    validate(
        physical,
        alternative,
        operands,
        displacement,
        &word.to_le_bytes(),
    )
}

pub(super) fn validate(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    displacement: u32,
    bytes: &[u8],
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    let [base, index, destination] = request(physical, alternative, operands, displacement)?;
    let word = bytes
        .try_into()
        .ok()
        .map(u32::from_le_bytes)
        .filter(|word| word & 0xffe0_fc00 == 0x3860_6800)
        .ok_or(Aarch64SelectedFormEncodingError::MalformedEncoding)?;
    if (word & 31) != u32::from(destination)
        || ((word >> 5) & 31) != u32::from(base)
        || ((word >> 16) & 31) != u32::from(index)
    {
        return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
    }
    Ok(ValidatedAarch64SelectedFormEncoding {
        bytes: bytes.to_vec(),
        footprint: Aarch64SelectedFormFootprint {
            register_reads: vec![operands[0], operands[1]],
            register_writes: vec![operands[2]],
            writes_nzcv: false,
            encoded: MachineEncodedEffects {
                external_operand_reads: vec![0, 1],
                external_operand_writes: vec![2],
                implicit_unit_uses: Vec::new(),
                implicit_unit_defs: Vec::new(),
                implicit_unit_clobbers: Vec::new(),
                memory: MachineEncodedMemoryEffect::ReadIndexedPointerV1 {
                    pointer_operand: 0,
                    index_operand: 1,
                    byte_count: 1,
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
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    displacement: u32,
) -> Result<[u8; 3], Aarch64SelectedFormEncodingError> {
    if physical.model() != &aarch64_physical_register_model()
        || displacement != 0
        || alternative
            != (MachineAlternativeKey {
                family: MachineAlternativeFamily::Load8Indexed,
                variant: 0,
            })
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
    fn indexed_byte_load_decodes_every_bit_and_operand_role() {
        let physical = register_model::validate_physical_register_model(
            crate::aarch64_physical_register_model(),
        )
        .unwrap();
        let operands = ["x1", "x2", "x3"].map(|name| physical.model().view_named(name).unwrap().id);
        let kind = SelectedInstructionKind::Load8Indexed;
        let alternative = MachineAlternativeKey {
            family: MachineAlternativeFamily::Load8Indexed,
            variant: 0,
        };
        let encoded =
            encode_aarch64_selected_memory_form(&physical, kind, alternative, &operands, 0)
                .unwrap();
        assert_eq!(encoded.bytes(), &0x3862_6823u32.to_le_bytes());
        assert_eq!(encoded.footprint().register_reads, operands[..2]);
        assert_eq!(encoded.footprint().register_writes, operands[2..]);
        assert_eq!(
            encoded.footprint().encoded.memory,
            MachineEncodedMemoryEffect::ReadIndexedPointerV1 {
                pointer_operand: 0,
                index_operand: 1,
                byte_count: 1,
            }
        );
        for bit in 0..encoded.bytes().len() * 8 {
            let mut corrupt = encoded.bytes().to_vec();
            corrupt[bit / 8] ^= 1 << (bit % 8);
            assert!(
                validate_aarch64_selected_memory_form(
                    &physical,
                    kind,
                    alternative,
                    &operands,
                    0,
                    &corrupt,
                )
                .is_err(),
                "accepted changed encoding bit {bit}"
            );
        }
        for role in 0..3 {
            let mut corrupt = operands;
            corrupt[role] = operands[(role + 1) % 3];
            assert!(
                validate_aarch64_selected_memory_form(
                    &physical,
                    kind,
                    alternative,
                    &corrupt,
                    0,
                    encoded.bytes(),
                )
                .is_err()
            );
        }
        assert!(
            validate_aarch64_selected_memory_form(
                &physical,
                kind,
                alternative,
                &operands,
                1,
                encoded.bytes(),
            )
            .is_err()
        );
        assert!(
            validate_aarch64_selected_memory_form(
                &physical,
                kind,
                alternative,
                &operands[..2],
                0,
                encoded.bytes(),
            )
            .is_err()
        );
        assert!(
            validate_aarch64_selected_memory_form(
                &physical,
                kind,
                MachineAlternativeKey {
                    family: MachineAlternativeFamily::Load64,
                    variant: 0,
                },
                &operands,
                0,
                encoded.bytes(),
            )
            .is_err()
        );
        for length in 0..encoded.bytes().len() {
            assert!(
                validate_aarch64_selected_memory_form(
                    &physical,
                    kind,
                    alternative,
                    &operands,
                    0,
                    &encoded.bytes()[..length],
                )
                .is_err()
            );
        }
        let mut trailing = encoded.bytes().to_vec();
        trailing.push(0);
        assert!(
            validate_aarch64_selected_memory_form(
                &physical,
                kind,
                alternative,
                &operands,
                0,
                &trailing,
            )
            .is_err()
        );
        let stack = physical.model().view_named("sp").unwrap().id;
        for role in 0..3 {
            let mut invalid = operands;
            invalid[role] = stack;
            assert!(
                encode_aarch64_selected_memory_form(&physical, kind, alternative, &invalid, 0,)
                    .is_err()
            );
        }
        // The load reads both inputs before defining its output.
        for result in operands[..2].iter().copied() {
            assert!(
                encode_aarch64_selected_memory_form(
                    &physical,
                    kind,
                    alternative,
                    &[operands[0], operands[1], result],
                    0,
                )
                .is_ok()
            );
        }
    }
}
