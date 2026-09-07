//! MOVZX r64, byte [base + index], with full-width runtime indexing.
use super::*;

pub(super) fn encode(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    displacement: u32,
) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
    let [base, index, destination] = request(physical, alternative, operands, displacement)?;
    // A fixed disp32 form covers every allocatable base, including rbp/r13.
    let bytes = [
        rex(destination, index, base),
        0x0f,
        0xb6,
        modrm(2, destination, 4),
        ((index & 7) << 3) | (base & 7),
        0,
        0,
        0,
        0,
    ];
    validate(physical, alternative, operands, displacement, &bytes)
}

pub(super) fn validate(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    displacement: u32,
    bytes: &[u8],
) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
    let [base, index, destination] = request(physical, alternative, operands, displacement)?;
    let [
        prefix,
        escape,
        opcode,
        mode,
        sib,
        offset0,
        offset1,
        offset2,
        offset3,
    ] = bytes
    else {
        return Err(X86_64SelectedFormEncodingError::MalformedEncoding);
    };
    let decoded_destination = ((mode >> 3) & 7) | ((prefix & 4) << 1);
    let decoded_base = (sib & 7) | ((prefix & 1) << 3);
    let decoded_index = ((sib >> 3) & 7) | ((prefix & 2) << 2);
    if prefix & 0xf8 != 0x48
        || *escape != 0x0f
        || *opcode != 0xb6
        || mode & 0xc7 != 0x84
        || sib >> 6 != 0
        || decoded_destination != destination
        || decoded_base != base
        || decoded_index != index
        || [*offset0, *offset1, *offset2, *offset3] != [0; 4]
    {
        return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
    }
    Ok(ValidatedX86_64SelectedFormEncoding {
        bytes: bytes.to_vec(),
        footprint: X86_64SelectedFormFootprint {
            register_reads: vec![operands[0], operands[1]],
            register_writes: vec![operands[2]],
            writes_rflags: false,
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
) -> Result<[u8; 3], X86_64SelectedFormEncodingError> {
    if physical.model() != &crate::x86_64_physical_register_model() {
        return Err(X86_64SelectedFormEncodingError::NonCanonicalPhysicalModel);
    }
    if displacement != 0
        || alternative
            != (MachineAlternativeKey {
                family: MachineAlternativeFamily::Load8Indexed,
                variant: 0,
            })
    {
        return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
    }
    resolve_registers(physical, operands)?
        .try_into()
        .map_err(|_| X86_64SelectedFormEncodingError::EncodedFormMismatch)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexed_byte_load_decodes_every_bit_and_operand_role() {
        let physical = register_model::validate_physical_register_model(
            crate::x86_64_physical_register_model(),
        )
        .unwrap();
        let operands =
            ["r13", "r12", "r15"].map(|name| physical.model().view_named(name).unwrap().id);
        let kind = SelectedInstructionKind::Load8Indexed;
        let alternative = MachineAlternativeKey {
            family: MachineAlternativeFamily::Load8Indexed,
            variant: 0,
        };
        let encoded =
            encode_x86_64_selected_memory_form(&physical, kind, alternative, &operands, 0).unwrap();
        assert_eq!(encoded.bytes(), &[0x4f, 0x0f, 0xb6, 0xbc, 0x25, 0, 0, 0, 0]);
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
                validate_x86_64_selected_memory_form(
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
                validate_x86_64_selected_memory_form(
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
            validate_x86_64_selected_memory_form(
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
            validate_x86_64_selected_memory_form(
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
            validate_x86_64_selected_memory_form(
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
                validate_x86_64_selected_memory_form(
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
            validate_x86_64_selected_memory_form(
                &physical,
                kind,
                alternative,
                &operands,
                0,
                &trailing,
            )
            .is_err()
        );
        let stack = physical.model().view_named("rsp").unwrap().id;
        for role in 0..3 {
            let mut invalid = operands;
            invalid[role] = stack;
            assert!(
                encode_x86_64_selected_memory_form(&physical, kind, alternative, &invalid, 0,)
                    .is_err()
            );
        }
        // The load reads both inputs before defining its output.
        for result in operands[..2].iter().copied() {
            assert!(
                encode_x86_64_selected_memory_form(
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
