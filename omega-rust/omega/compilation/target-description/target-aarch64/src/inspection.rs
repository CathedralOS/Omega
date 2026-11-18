//! Read final instruction bytes and check the selected operand contract.
use ::register_model::{RegisterOperandAccess, RegisterViewId, ValidatedPhysicalRegisterModel};
use selected_instructions::{
    MachineAlternativeKey, MachineEncodedEffects, SelectedInstructionKind,
    SelectedNormalizedForeignCall,
};
use target::NativeTarget;

pub fn decode_address_words(words: &[u32], destination: u8, base: u8) -> Option<u64> {
    let mut offset = 0_u64;
    let mut previous_pages = None;
    let mut remainder_seen = false;
    for (position, word) in words.iter().copied().enumerate() {
        let source = if position == 0 { base } else { destination };
        if word & 0xff80_0000 != 0x9100_0000
            || word & 31 != u32::from(destination)
            || (word >> 5) & 31 != u32::from(source)
            || remainder_seen
        {
            return None;
        }
        let immediate = u64::from((word >> 10) & 4095);
        if word & (1 << 22) != 0 {
            if immediate == 0 || previous_pages.is_some_and(|pages| pages != 4095) {
                return None;
            }
            previous_pages = Some(immediate);
            offset = offset.checked_add(immediate << 12)?;
        } else {
            if immediate == 0 && position != 0 {
                return None;
            }
            remainder_seen = true;
            offset = offset.checked_add(immediate)?;
        }
    }
    (!words.is_empty()).then_some(offset)
}

pub fn decode_aarch64_selected_hosted_read_byte(target: NativeTarget, bytes: &[u8]) -> Option<u32> {
    let words = words(bytes)?;
    let reach = words
        .iter()
        .take_while(|word| *word & 0xff80_001f == 0x9100_0001)
        .count();
    let address = &words[..reach];
    let payload = u32::try_from(decode_address_words(address, 1, 31)?).ok()?;
    if !payload.is_multiple_of(4) || payload < 4 {
        return None;
    }
    let reach = u32::try_from(reach).ok()?;
    let (calls, readdress): (&[u32], bool) = if target == NativeTarget::linux_arm64() {
        (
            &[
                0xd280_0000,
                0xd280_0022,
                0xd280_07e8,
                0xd400_0001,
                0xb400_00e0,
                0xf100_041f,
                0x5400_0081,
                0x5280_0029,
            ],
            false,
        )
    } else if target == NativeTarget::macos_arm64() {
        (
            &[
                0xd280_0000,
                0xd280_0022,
                0xd280_0070,
                0xd400_1001,
                0x5400_0002 | ((7 + reach) << 5),
                0xb400_0000 | ((7 + reach) << 5),
                0xf100_041f,
                0x5400_0001 | ((4 + reach) << 5),
                0x5280_0029,
            ],
            true,
        )
    } else {
        return None;
    };
    let mut expected = address.to_vec();
    expected.extend([0xb81f_c03f, 0xb900_003f]);
    expected.extend_from_slice(calls);
    if readdress {
        expected.extend_from_slice(address);
    }
    expected.extend([0xb81f_c029, 0x1400_0002, 0xd420_0000]);
    (words == expected).then_some(payload - 4)
}

#[allow(clippy::too_many_arguments)]
pub fn selected_system_call_effects(
    target: NativeTarget,
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operand_views: &[RegisterViewId],
    operand_accesses: &[RegisterOperandAccess],
    number: i64,
    bytes: &[u8],
) -> Option<MachineEncodedEffects> {
    if target != NativeTarget::linux_arm64()
        || number < 0
        || physical.identity() != crate::canonical_aarch64_physical_register_model_identity()
        || !matches!(kind, SelectedInstructionKind::NormalizedForeignCall { .. })
        || alternative != kind.call_alternative_key()
    {
        return None;
    }
    if bytes.len() != 20 {
        return None;
    }
    let mut decoded_number = 0u64;
    for (halfword, chunk) in bytes[..16].as_chunks::<4>().0.iter().enumerate() {
        let word = u32::from_le_bytes(*chunk);
        let opcode = if halfword == 0 {
            0xd280_0000
        } else {
            0xf280_0000
        };
        if word & !0x001f_ffe0 != opcode | ((halfword as u32) << 21) | 8 {
            return None;
        }
        decoded_number |= u64::from((word >> 5) & 0xffff) << (halfword * 16);
    }
    if bytes[16..] != [0x01, 0x00, 0x00, 0xd4] || decoded_number != number as u64 {
        return None;
    }
    let catalog = crate::aarch64_register_constraint_catalog(physical);
    let keys = crate::aarch64_linux_system_call_keys();
    let row = catalog.row_matching(&keys, operand_views, operand_accesses)?;
    Some(MachineEncodedEffects::supervisor_call_v1(row))
}

pub fn selected_table_call_effects(
    record: &SelectedNormalizedForeignCall,
    alternative: MachineAlternativeKey,
    views: &[RegisterViewId],
    accesses: &[RegisterOperandAccess],
    bytes: &[u8],
) -> Option<MachineEncodedEffects> {
    let (base, offset) = crate::form_effects::table_call::table_address(record)?;
    let (&[low, high, add, load, call], []) = bytes.as_chunks::<4>() else {
        return None;
    };
    let [low, high, add, load, call] = [low, high, add, load, call].map(u32::from_le_bytes);
    if low & 0xffe0_001f != 0xd280_0011
        || high & 0xffe0_001f != 0xf2a0_0011
        || ((low >> 5) & 0xffff) != offset & 0xffff
        || ((high >> 5) & 0xffff) != offset >> 16
        || add != 0x8b11_0011 | (u32::from(base) << 5)
        || load != 0xf940_0231
        || call != 0xd63f_0220
    {
        return None;
    }
    let row = crate::machine_effects::normalized_foreign_constraint(record)?;
    if views.len() != row.operands.len()
        || accesses.len() != row.operands.len()
        || row
            .operands
            .iter()
            .zip(views.iter().zip(accesses))
            .any(|(operand, (view, access))| {
                operand.fixed_view != Some(*view) || operand.access != *access
            })
    {
        return None;
    }
    let declaration = crate::aarch64_normalized_foreign_call_declaration(record, &row)?;
    let [form] = declaration.alternatives.as_slice() else {
        return None;
    };
    if form.key != alternative {
        return None;
    }
    Some(form.encoded.clone())
}

/// Return the stack slots addressed by the MRS/STR and LDR/MSR sequences.
/// A frame slot beyond the scaled immediate uses canonical page steps.
pub fn floating_control_slots(save: &[u8], restore: &[u8]) -> Option<(u32, u32)> {
    let (mrs, save_access) = save.split_first_chunk::<4>()?;
    let (restore_access, msr) = restore.split_last_chunk::<4>()?;
    if u32::from_le_bytes(*mrs) != 0xd53b_4409 || u32::from_le_bytes(*msr) != 0xd51b_4409 {
        return None;
    }
    Some((
        floating_stack_slot(save_access, 0xf900_0009, 16)?,
        floating_stack_slot(restore_access, 0xf940_0009, 9)?,
    ))
}

fn floating_stack_slot(bytes: &[u8], opcode: u32, address_register: u8) -> Option<u32> {
    let (prefix, access) = bytes.split_last_chunk::<4>()?;
    let word = u32::from_le_bytes(*access);
    if word & 0xffc0_001f != opcode {
        return None;
    }
    let base = (word >> 5) & 31;
    let low = ((word >> 10) & 4095) * 8;
    if prefix.is_empty() {
        return (base == 31).then_some(low);
    }
    if base != u32::from(address_register) || low >= 4096 {
        return None;
    }
    let (steps, remainder) = prefix.as_chunks::<4>();
    if !remainder.is_empty() {
        return None;
    }
    let words = steps
        .iter()
        .map(|step| u32::from_le_bytes(*step))
        .collect::<Vec<_>>();
    if words.iter().any(|word| word & (1 << 22) == 0) {
        return None;
    }
    let pages = u32::try_from(decode_address_words(&words, address_register, 31)?).ok()?;
    let slot = pages.checked_add(low)?;
    (slot > 4095 * 8).then_some(slot)
}

pub fn decode_hosted_exit_process_register(target: NativeTarget, bytes: &[u8]) -> Option<u8> {
    if ![NativeTarget::linux_arm64(), NativeTarget::macos_arm64()].contains(&target) {
        return None;
    }
    let (&[source, number, service, trap], []) = bytes.as_chunks::<4>() else {
        return None;
    };
    let words = [source, number, service, trap].map(u32::from_le_bytes);
    let register = ((words[0] >> 16) & 31) as u8;
    if words[0] & !0x001f_0000 != 0x2a00_03e0
        || register == 31
        || words[1]
            != if target == NativeTarget::macos_arm64() {
                0xd280_0030
            } else {
                0xd280_0bc8
            }
        || words[2]
            != if target == NativeTarget::macos_arm64() {
                0xd400_1001
            } else {
                0xd400_0001
            }
        || words[3] != 0xd420_0000
    {
        return None;
    }
    Some(register)
}

pub fn decode_aarch64_selected_hosted_exit_process_i32(
    target: NativeTarget,
    bytes: &[u8],
) -> Option<calling_conventions::MachineRegister> {
    let register = decode_hosted_exit_process_register(target, bytes)?;
    Some(calling_conventions::MachineRegister::Aarch64X(register))
}

pub fn aarch64_hosted_write_byte_i32_byte_count(
    target: NativeTarget,
    displacement: u32,
) -> Option<usize> {
    let address = if displacement <= 4095 {
        0
    } else {
        crate::form_effects::address_word_count(u64::from(displacement)) as usize
    };
    let service = if target == NativeTarget::linux_arm64() {
        5
    } else if target == NativeTarget::macos_arm64() {
        6
    } else {
        return None;
    };
    Some((4 + address + service) * 4)
}

pub fn decode_hosted_write_byte_register(target: NativeTarget, bytes: &[u8]) -> Option<(u8, u32)> {
    let words = words(bytes)?;
    let tail: &[u32] = if target == NativeTarget::linux_arm64() {
        &[
            0xd280_0808,
            0xd400_0001,
            0xf100_001f,
            0x5400_004c,
            0xd420_0000,
        ]
    } else if target == NativeTarget::macos_arm64() {
        &[
            0xd280_0090,
            0xd400_1001,
            0x5400_0062,
            0xf100_041f,
            0x5400_0040,
            0xd420_0000,
        ]
    } else {
        return None;
    };
    let prefix = words.strip_suffix(tail)?;
    let (register, displacement) = match prefix {
        [store, 0xd280_0020, address, 0xd280_0022] if store & 0xffc0_03e0 == 0x3900_03e0 => {
            let displacement = (store >> 10) & 4095;
            if *address != 0x9100_03e1 | (displacement << 10) {
                return None;
            }
            (store & 31, displacement)
        }
        [copy, steps @ .., 0x3900_0022, 0xd280_0020, 0xd280_0022]
            if copy & 0xffe0_ffff == 0x2a00_03e2 =>
        {
            let displacement = u32::try_from(decode_address_words(steps, 1, 31)?).ok()?;
            if displacement <= 4095 {
                return None;
            }
            ((copy >> 16) & 31, displacement)
        }
        _ => return None,
    };
    (register < 31).then_some((register as u8, displacement))
}

pub fn decode_aarch64_selected_hosted_write_byte_i32(
    target: NativeTarget,
    bytes: &[u8],
) -> Option<(calling_conventions::MachineRegister, u32)> {
    let (register, displacement) = decode_hosted_write_byte_register(target, bytes)?;
    Some((
        calling_conventions::MachineRegister::Aarch64X(register),
        displacement,
    ))
}

fn words(bytes: &[u8]) -> Option<Vec<u32>> {
    let (chunks, tail) = bytes.as_chunks::<4>();
    tail.is_empty().then(|| {
        chunks
            .iter()
            .map(|word| u32::from_le_bytes(*word))
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use super::floating_control_slots;

    #[test]
    fn floating_control_replay_checks_both_directions_and_the_actual_frame_slot() {
        use isa_aarch64::{
            encode_restore_fpcr_from_sp_displacement as restore,
            encode_save_fpcr_to_sp_displacement as save,
        };
        let slots = [
            0,
            8,
            32760,
            32768,
            65544,
            4095 * 4096,
            4095 * 4096 + 8192,
            u32::MAX - 7,
        ];
        for slot in slots {
            let saved = save(slot).unwrap();
            let restored = restore(slot).unwrap();
            assert_eq!(
                floating_control_slots(&saved, &restored),
                Some((slot, slot))
            );
            assert_eq!(floating_control_slots(&restored, &saved), None);
            for (is_save, original) in [(true, &saved), (false, &restored)] {
                for bit in 0..original.len() * 8 {
                    let mut changed = original.clone();
                    changed[bit / 8] ^= 1 << (bit % 8);
                    let actual = if is_save {
                        floating_control_slots(&changed, &restored)
                    } else {
                        floating_control_slots(&saved, &changed)
                    };
                    assert_ne!(actual, Some((slot, slot)), "slot {slot}, bit {bit}");
                }
            }
        }
        // Two small page steps describe the same address as one full page operand
        // but are not the canonical instruction sequence for that address.
        let split = [0xd53b_4409, 0x9140_13f0, 0x9140_1210, 0xf900_0209u32]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>();
        assert_eq!(
            floating_control_slots(&split, &restore(32768).unwrap()),
            None
        );
    }
}
