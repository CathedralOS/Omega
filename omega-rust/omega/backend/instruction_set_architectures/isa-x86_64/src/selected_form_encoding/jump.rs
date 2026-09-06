//! Ordinary unconditional rel32 control; no hidden branch relaxation.

use super::*;

pub fn encode_x86_64_selected_jump_form(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    displacement: i64,
) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
    let displacement = i32::try_from(displacement)
        .map_err(|_| X86_64SelectedFormEncodingError::EncodedFormMismatch)?;
    let mut bytes = vec![0xe9];
    bytes.extend_from_slice(&displacement.to_le_bytes());
    validate_x86_64_selected_jump_form(physical, alternative, i64::from(displacement), &bytes)
}

pub fn validate_x86_64_selected_jump_form(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    displacement: i64,
    bytes: &[u8],
) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
    if physical.model() != &x86_64_physical_register_model() {
        return Err(X86_64SelectedFormEncodingError::NonCanonicalPhysicalModel);
    }
    if alternative
        != (MachineAlternativeKey {
            family: MachineAlternativeFamily::Jump,
            variant: 0,
        })
    {
        return Err(X86_64SelectedFormEncodingError::AlternativeMismatch);
    }
    let [0xe9, a, b, c, d] = bytes else {
        return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
    };
    if i64::from(i32::from_le_bytes([*a, *b, *c, *d])) != displacement {
        return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
    }
    let pc = physical
        .model()
        .view_named("rip")
        .expect("canonical model has RIP")
        .units
        .clone();
    Ok(ValidatedX86_64SelectedFormEncoding {
        bytes: bytes.to_vec(),
        footprint: X86_64SelectedFormFootprint {
            register_reads: vec![],
            register_writes: vec![],
            writes_rflags: false,
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
            register_model::validate_physical_register_model(x86_64_physical_register_model())
                .unwrap();
        let key = MachineAlternativeKey {
            family: MachineAlternativeFamily::Jump,
            variant: 0,
        };
        for displacement in [i64::from(i32::MIN), -5, 0, 7, i64::from(i32::MAX)] {
            let encoded = encode_x86_64_selected_jump_form(&physical, key, displacement).unwrap();
            assert_eq!(encoded.bytes().len(), 5);
            assert!(
                validate_x86_64_selected_jump_form(
                    &physical,
                    key,
                    displacement + 1,
                    encoded.bytes()
                )
                .is_err()
            );
            let mut corrupt = encoded.bytes().to_vec();
            corrupt[0] ^= 1;
            assert!(
                validate_x86_64_selected_jump_form(&physical, key, displacement, &corrupt).is_err()
            );
            assert_eq!(
                encoded.footprint().encoded.control,
                MachineEncodedControlEffect::UnconditionalRelativeBranchV1
            );
        }
        assert!(encode_x86_64_selected_jump_form(&physical, key, i64::from(i32::MAX) + 1).is_err());
    }
}
