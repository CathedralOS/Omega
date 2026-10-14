//! A fixed local loop writes zero over a runtime span without source CFG
//! edges. Destination and count survive; the early-clobber cursor owns the
//! only register write, and every stored byte comes from the zero register.

use super::{Aarch64SelectedFormEncodingError, ValidatedAarch64SelectedFormEncoding};
use crate::selected_form_encoding::selected_forms::resolve_registers;
use register_model::RegisterViewId;
use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions::MachineAlternativeFamily;
use selected_instructions::MachineAlternativeKey;
use selected_instructions::MachineEncodedMemoryEffect;

/// `strb wzr, [destination, cursor]`: register-offset byte store whose
/// source field names the zero register.
const ZERO_REGISTER: u32 = 31;

fn request(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
) -> Result<[u8; 3], Aarch64SelectedFormEncodingError> {
    if physical.identity() != crate::canonical_aarch64_physical_register_model_identity()
        || alternative
            != (MachineAlternativeKey {
                family: MachineAlternativeFamily::ZeroBytes,
                variant: 0,
            })
    {
        return Err(Aarch64SelectedFormEncodingError::AlternativeMismatch);
    }
    let registers: [u8; 3] = resolve_registers(physical, operands)?
        .try_into()
        .map_err(|_| Aarch64SelectedFormEncodingError::EncodedFormMismatch)?;
    // The cursor is written before either input is read, so it may not share
    // a home with the destination or the count.
    if registers[..2].contains(&registers[2]) {
        return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
    }
    Ok(registers)
}

pub(super) fn encode(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    let [destination, count, cursor] = request(physical, alternative, operands)?.map(u32::from);
    let words = [
        0xd280_0000 | cursor, // mov cursor, #0
        0xb400_00a0 | count,  // cbz count, end (+20)
        0x3820_6800 | (cursor << 16) | (destination << 5) | ZERO_REGISTER,
        0x9100_0400 | (cursor << 5) | cursor, // add cursor, cursor, #1
        0xeb00_001f | (count << 16) | (cursor << 5), // cmp cursor, count
        0x54ff_ffa1,                          // b.ne store (-12)
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
    let [destination, count, cursor] = request(physical, alternative, operands)?.map(u32::from);
    let (word_bytes, tail) = bytes.as_chunks::<4>();
    let word_bytes: &[[u8; 4]; 6] = word_bytes
        .try_into()
        .map_err(|_| Aarch64SelectedFormEncodingError::MalformedEncoding)?;
    if !tail.is_empty() {
        return Err(Aarch64SelectedFormEncodingError::MalformedEncoding);
    }
    let [initialize, skip, store, advance, compare, repeat] = word_bytes.map(u32::from_le_bytes);
    // Decode the opcode, register fields and both local branch displacements.
    // The guard skips every access for a zero count; the cursor reaches
    // count before another store, and count MAX does not overflow it.
    if initialize & !31 != 0xd280_0000
        || initialize & 31 != cursor
        || skip & 0xff00_0000 != 0xb400_0000
        || skip & 31 != count
        || (skip >> 5) & 0x7ffff != 5
        || store & 0xffe0_fc00 != 0x3820_6800
        || store & 31 != ZERO_REGISTER
        || (store >> 5) & 31 != destination
        || (store >> 16) & 31 != cursor
        || advance & !0x3ff != 0x9100_0400
        || advance & 31 != cursor
        || (advance >> 5) & 31 != cursor
        || compare & 0xffe0_fc1f != 0xeb00_001f
        || (compare >> 5) & 31 != cursor
        || (compare >> 16) & 31 != count
        || repeat != 0x54ff_ffa1
    {
        return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
    }
    super::nzcv_effects_encoding(
        physical,
        operands,
        bytes,
        2,
        MachineEncodedMemoryEffect::ZeroBytesV1 {
            destination_pointer_operand: 0,
            count_operand: 1,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::{encode, validate};
    use crate::aarch64_physical_register_model;
    use selected_instructions::MachineAlternativeFamily;
    use selected_instructions::MachineAlternativeKey;

    #[test]
    fn zero_loop_replays_every_bit_and_rejects_cursor_aliases() {
        let physical =
            register_model::validate_physical_register_model(aarch64_physical_register_model())
                .unwrap();
        let operands = ["x0", "x1", "x2"].map(|name| physical.model().view_named(name).unwrap().id);
        let alternative = MachineAlternativeKey {
            family: MachineAlternativeFamily::ZeroBytes,
            variant: 0,
        };
        let encoded = encode(&physical, alternative, &operands).unwrap();
        assert_eq!(
            encoded.bytes,
            [
                0xd280_0002_u32, // mov x2, #0
                0xb400_00a1,     // cbz x1, +20
                0x3822_681f,     // strb wzr, [x0, x2]
                0x9100_0442,     // add x2, x2, #1
                0xeb01_005f,     // cmp x2, x1
                0x54ff_ffa1,     // b.ne -12
            ]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>()
        );
        assert_eq!(encoded.footprint.register_reads, operands[..2]);
        assert_eq!(encoded.footprint.register_writes, operands[2..]);
        for position in 0..encoded.bytes.len() * 8 {
            let mut changed = encoded.bytes.clone();
            changed[position / 8] ^= 1 << (position % 8);
            assert!(
                validate(&physical, alternative, &operands, &changed).is_err(),
                "bit {position}"
            );
        }
        for length in 0..encoded.bytes.len() {
            assert!(validate(&physical, alternative, &operands, &encoded.bytes[..length]).is_err());
        }
        for input in 0..2 {
            let mut changed = operands;
            changed[2] = changed[input];
            assert!(encode(&physical, alternative, &changed).is_err());
        }
        let copy = MachineAlternativeKey {
            family: MachineAlternativeFamily::CopyBytes,
            variant: 0,
        };
        assert!(encode(&physical, copy, &operands).is_err());
    }
}
