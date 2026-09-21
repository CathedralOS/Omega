//! A fixed local loop implements a runtime span without source CFG edges.
//! Inputs survive; early-clobber cursor and byte temporaries own every write.

use super::{
    Aarch64SelectedFormEncodingError, Aarch64SelectedFormFootprint,
    ValidatedAarch64SelectedFormEncoding,
};
use crate::selected_form_encoding::selected_forms::resolve_registers;
use register_model::RegisterViewId;
use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions::MachineAlternativeFamily;
use selected_instructions::MachineAlternativeKey;
use selected_instructions::MachineEncodedControlEffect;
use selected_instructions::MachineEncodedEffects;
use selected_instructions::MachineEncodedMemoryEffect;
use selected_instructions::MachineEncodedStackEffect;
use selected_instructions::MachineEncodedTrapBehavior;
fn request(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
) -> Result<[u8; 5], Aarch64SelectedFormEncodingError> {
    if physical.identity() != crate::canonical_aarch64_physical_register_model_identity()
        || alternative
            != (MachineAlternativeKey {
                family: MachineAlternativeFamily::CopyBytes,
                variant: 0,
            })
    {
        return Err(Aarch64SelectedFormEncodingError::AlternativeMismatch);
    }
    let registers: [u8; 5] = resolve_registers(physical, operands)?
        .try_into()
        .map_err(|_| Aarch64SelectedFormEncodingError::EncodedFormMismatch)?;
    if registers[3] == registers[4]
        || registers[..3].contains(&registers[3])
        || registers[..3].contains(&registers[4])
    {
        return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
    }
    Ok(registers)
}

pub(super) fn encode(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    let [source, destination, count, cursor, byte] =
        request(physical, alternative, operands)?.map(u32::from);
    let words = [
        0xd280_0000 | cursor, // mov cursor, #0
        0xb400_00c0 | count,  // cbz count, end (+24)
        0x3860_6800 | (cursor << 16) | (source << 5) | byte,
        0x3820_6800 | (cursor << 16) | (destination << 5) | byte,
        0x9100_0400 | (cursor << 5) | cursor,
        0xeb00_001f | (count << 16) | (cursor << 5),
        0x54ff_ff81, // b.ne load (-16)
    ];
    let bytes = words
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect::<Vec<_>>();
    validate(physical, alternative, operands, &bytes)
}

pub(super) fn validate(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    bytes: &[u8],
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    let [source, destination, count, cursor, byte] =
        request(physical, alternative, operands)?.map(u32::from);
    let (word_bytes, tail) = bytes.as_chunks::<4>();
    let word_bytes: &[[u8; 4]; 7] = word_bytes
        .try_into()
        .map_err(|_| Aarch64SelectedFormEncodingError::MalformedEncoding)?;
    if !tail.is_empty() {
        return Err(Aarch64SelectedFormEncodingError::MalformedEncoding);
    }
    let [initialize, skip, load, store, advance, compare, repeat] =
        word_bytes.map(u32::from_le_bytes);
    // Decode the opcode, register fields and both local branch displacements.
    // cursor reaches count before another access; count MAX does not overflow.
    if initialize & !31 != 0xd280_0000
        || initialize & 31 != cursor
        || skip & 0xff00_0000 != 0xb400_0000
        || skip & 31 != count
        || (skip >> 5) & 0x7ffff != 6
        || load & 0xffe0_fc00 != 0x3860_6800
        || load & 31 != byte
        || (load >> 5) & 31 != source
        || (load >> 16) & 31 != cursor
        || store & 0xffe0_fc00 != 0x3820_6800
        || store & 31 != byte
        || (store >> 5) & 31 != destination
        || (store >> 16) & 31 != cursor
        || advance & !0x3ff != 0x9100_0400
        || advance & 31 != cursor
        || (advance >> 5) & 31 != cursor
        || compare & 0xffe0_fc1f != 0xeb00_001f
        || (compare >> 5) & 31 != cursor
        || (compare >> 16) & 31 != count
        || repeat != 0x54ff_ff81
    {
        return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
    }
    let flags = physical
        .model()
        .view_named("nzcv")
        .ok_or(Aarch64SelectedFormEncodingError::EncodedFormMismatch)?
        .units
        .clone();
    Ok(ValidatedAarch64SelectedFormEncoding {
        bytes: bytes.to_vec(),
        footprint: Aarch64SelectedFormFootprint {
            register_reads: operands[..3].to_vec(),
            register_writes: operands[3..].to_vec(),
            writes_nzcv: true,
            encoded: MachineEncodedEffects {
                external_operand_reads: vec![0, 1, 2],
                external_operand_writes: vec![3, 4],
                implicit_unit_uses: vec![],
                implicit_unit_defs: vec![],
                implicit_unit_clobbers: flags,
                memory: MachineEncodedMemoryEffect::CopyBytesV1 {
                    source_pointer_operand: 0,
                    destination_pointer_operand: 1,
                    count_operand: 2,
                },
                stack: MachineEncodedStackEffect::UnchangedV1,
                trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
                control: MachineEncodedControlEffect::FallThroughV1,
            },
        },
    })
}

#[cfg(test)]
mod tests {
    use super::{encode, validate};
    use crate::aarch64_physical_register_model;
    use selected_instructions::MachineAlternativeFamily;
    use selected_instructions::MachineAlternativeKey;

    #[test]
    fn copy_loop_replays_every_bit_and_rejects_scratch_aliases() {
        let physical =
            register_model::validate_physical_register_model(aarch64_physical_register_model())
                .unwrap();
        let operands = ["x0", "x1", "x2", "x3", "x4"]
            .map(|name| physical.model().view_named(name).unwrap().id);
        let alternative = MachineAlternativeKey {
            family: MachineAlternativeFamily::CopyBytes,
            variant: 0,
        };
        let encoded = encode(&physical, alternative, &operands).unwrap();
        for position in 0..encoded.bytes.len() * 8 {
            let mut changed = encoded.bytes.clone();
            changed[position / 8] ^= 1 << (position % 8);
            assert!(
                validate(&physical, alternative, &operands, &changed).is_err(),
                "bit {position}"
            );
        }
        for scratch in 3..5 {
            for input in 0..5 {
                if input == scratch {
                    continue;
                }
                let mut changed = operands;
                changed[scratch] = changed[input];
                assert!(encode(&physical, alternative, &changed).is_err());
            }
        }
    }
}
