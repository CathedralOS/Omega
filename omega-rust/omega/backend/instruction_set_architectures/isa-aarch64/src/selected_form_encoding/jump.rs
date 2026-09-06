//! Ordinary unconditional B immediate control.

use super::*;

pub fn encode_aarch64_selected_jump_form(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    displacement: i64,
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    if displacement % 4 != 0 {
        return Err(Aarch64SelectedFormEncodingError::BranchDisplacementMisaligned);
    }
    let words = displacement / 4;
    if !(-(1_i64 << 25)..(1_i64 << 25)).contains(&words) {
        return Err(Aarch64SelectedFormEncodingError::BranchDisplacementOutsideImm26);
    }
    let word = 0x1400_0000 | ((words as u32) & 0x03ff_ffff);
    validate_aarch64_selected_jump_form(physical, alternative, displacement, &word.to_le_bytes())
}

pub fn validate_aarch64_selected_jump_form(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    displacement: i64,
    bytes: &[u8],
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    if physical.model() != &aarch64_physical_register_model() {
        return Err(Aarch64SelectedFormEncodingError::NonCanonicalPhysicalModel);
    }
    if alternative
        != (MachineAlternativeKey {
            family: MachineAlternativeFamily::Jump,
            variant: 0,
        })
    {
        return Err(Aarch64SelectedFormEncodingError::AlternativeMismatch);
    }
    let bytes: [u8; 4] = bytes
        .try_into()
        .map_err(|_| Aarch64SelectedFormEncodingError::MalformedEncoding)?;
    let word = u32::from_le_bytes(bytes);
    let actual = i64::from(((word & 0x03ff_ffff) << 6) as i32 >> 6) * 4;
    if word & 0xfc00_0000 != 0x1400_0000 || actual != displacement {
        return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
    }
    let pc = physical
        .model()
        .view_named("pc")
        .expect("canonical model has PC")
        .units
        .clone();
    Ok(ValidatedAarch64SelectedFormEncoding {
        bytes: bytes.to_vec(),
        footprint: Aarch64SelectedFormFootprint {
            register_reads: vec![],
            register_writes: vec![],
            writes_nzcv: false,
            encoded: MachineEncodedEffects {
                external_operand_reads: vec![],
                external_operand_writes: vec![],
                implicit_unit_uses: pc.clone(),
                implicit_unit_defs: pc,
                implicit_unit_clobbers: vec![],
                memory: MachineEncodedMemoryEffect::NoneV1,
                stack: MachineEncodedStackEffect::UnchangedV1,
                trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
                control: MachineEncodedControlEffect::UnconditionalRelativeBranchV1,
            },
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn jump_round_trip_rejects_wrong_target_and_opcode() {
        let physical =
            register_model::validate_physical_register_model(aarch64_physical_register_model())
                .unwrap();
        let key = MachineAlternativeKey {
            family: MachineAlternativeFamily::Jump,
            variant: 0,
        };
        for displacement in [-134217728, -4, 0, 8, 134217724] {
            let encoded = encode_aarch64_selected_jump_form(&physical, key, displacement).unwrap();
            assert_eq!(encoded.bytes().len(), 4);
            assert!(
                validate_aarch64_selected_jump_form(
                    &physical,
                    key,
                    displacement + 4,
                    encoded.bytes()
                )
                .is_err()
            );
            let mut corrupt = encoded.bytes().to_vec();
            corrupt[3] ^= 0x80;
            assert!(
                validate_aarch64_selected_jump_form(&physical, key, displacement, &corrupt)
                    .is_err()
            );
            assert_eq!(
                encoded.footprint().encoded.control,
                MachineEncodedControlEffect::UnconditionalRelativeBranchV1
            );
        }
        assert!(encode_aarch64_selected_jump_form(&physical, key, 134217728).is_err());
        assert!(encode_aarch64_selected_jump_form(&physical, key, 1).is_err());
    }
}
