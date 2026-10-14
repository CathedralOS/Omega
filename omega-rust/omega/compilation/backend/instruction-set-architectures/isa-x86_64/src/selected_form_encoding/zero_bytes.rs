//! Exact zero fill over a runtime span with one explicit cursor scratch and
//! local control flow, the same encoding style as the `CopyBytes` loop. The
//! stored byte is an immediate, so no register carries the zero.

use super::{
    MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedMemoryEffect, RegisterViewId,
    ValidatedPhysicalRegisterModel, ValidatedX86_64SelectedFormEncoding,
    X86_64SelectedFormEncodingError,
};
use crate::selected_form_encoding::copy_bytes::register_instruction;
use crate::selected_form_encoding::instruction_bytes::{modrm, rex};
use crate::selected_form_encoding::request_validation::resolve_registers;

/// xor(3) + test(3) + jz(2) + store(9) + inc(3) + cmp(3) + jb(2).
const ENCODED_BYTES: usize = 25;
/// The loop head: the first byte of the zero store.
const STORE: usize = 8;

pub(super) fn encode(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
    let [destination, count, cursor] = request(physical, alternative, operands)?;
    let mut bytes = Vec::with_capacity(ENCODED_BYTES);
    // xor cursor, cursor
    bytes.extend_from_slice(&[rex(cursor, 0, cursor), 0x31, modrm(3, cursor, cursor)]);
    // test count, count; jz end
    bytes.extend_from_slice(&[
        rex(count, 0, count),
        0x85,
        modrm(3, count, count),
        0x74,
        (ENCODED_BYTES - STORE) as u8,
    ]);
    // mov byte [destination + cursor + 0], 0 — REX without W: C6 /0 is a byte store.
    bytes.extend_from_slice(&[
        rex(0, cursor, destination) & !8,
        0xc6,
        modrm(2, 0, 4),
        ((cursor & 7) << 3) | (destination & 7),
        0,
        0,
        0,
        0,
        0,
    ]);
    // inc cursor
    bytes.extend_from_slice(&[rex(0, 0, cursor), 0xff, modrm(3, 0, cursor)]);
    // cmp cursor, count; jb store
    bytes.extend_from_slice(&[
        rex(count, 0, cursor),
        0x39,
        modrm(3, count, cursor),
        0x72,
        (STORE as i8 - ENCODED_BYTES as i8) as u8,
    ]);
    validate(physical, alternative, operands, &bytes)
}

pub(super) fn validate(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    bytes: &[u8],
) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
    let [destination, count, cursor] = request(physical, alternative, operands)?;
    // Decode roles and exact branch destinations, independently of
    // construction. The zero guard jumps past the store; the backedge
    // targets it while the cursor stays below count.
    if bytes.len() != ENCODED_BYTES
        || !register_instruction(&bytes[0..3], 0x31, cursor, cursor)
        || !register_instruction(&bytes[3..6], 0x85, count, count)
        || bytes[6] != 0x74
        || 8_i16 + i16::from(bytes[7] as i8) != ENCODED_BYTES as i16
        || !zero_store(&bytes[STORE..17], destination, cursor)
        || bytes[17] & 0xfe != 0x48
        || bytes[18] != 0xff
        || bytes[19] & 0xf8 != 0xc0
        || (bytes[19] & 7) | ((bytes[17] & 1) << 3) != cursor
        || !register_instruction(&bytes[20..23], 0x39, count, cursor)
        || bytes[23] != 0x72
        || ENCODED_BYTES as i16 + i16::from(bytes[24] as i8) != STORE as i16
    {
        return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
    }
    super::canonical_effects_encoding(
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

/// `mov byte [base + cursor*1 + disp32], imm8` with a zero displacement and
/// a zero immediate; REX.W stays clear because the store is one byte.
fn zero_store(bytes: &[u8], base: u8, cursor: u8) -> bool {
    let [prefix, 0xc6, mode, sib, displacement @ .., 0] = bytes else {
        return false;
    };
    *prefix & 0xf8 == 0x40
        && *prefix & 4 == 0
        && *mode == 0x84
        && sib >> 6 == 0
        && displacement == [0; 4]
        && (sib & 7) | ((prefix & 1) << 3) == base
        && ((sib >> 3) & 7) | ((prefix & 2) << 2) == cursor
}

fn request(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
) -> Result<[u8; 3], X86_64SelectedFormEncodingError> {
    super::request_validation::guard_canonical(physical)?;
    if alternative
        != (MachineAlternativeKey {
            family: MachineAlternativeFamily::ZeroBytes,
            variant: 0,
        })
    {
        return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
    }
    let registers: [u8; 3] = resolve_registers(physical, operands)?
        .try_into()
        .map_err(|_| X86_64SelectedFormEncodingError::OperandCountMismatch)?;
    // The cursor is written before either input is read.
    if registers[..2].contains(&registers[2]) {
        return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
    }
    Ok(registers)
}

#[cfg(test)]
mod tests {
    use super::super::{
        SelectedInstructionKind, encode_x86_64_selected_form,
        validate_x86_64_selected_form_encoding,
    };
    use super::{MachineAlternativeFamily, MachineAlternativeKey, encode, validate};

    #[test]
    fn zero_bytes_catalog_requires_the_early_clobber_cursor_and_exact_dynamic_effect() {
        let physical = register_model::validate_physical_register_model(
            crate::x86_64_physical_register_model(),
        )
        .unwrap();
        let catalog = crate::x86_64_register_constraint_catalog(&physical);
        let constraints =
            crate::validate_x86_64_register_constraint_catalog(catalog.clone(), &physical).unwrap();
        crate::selected_form_encoding::tests::assert_early_clobber_required(
            &catalog,
            &physical,
            crate::X86_64_ZERO_BYTES,
            &[2],
        );
        for target in [
            target::NativeTarget::linux_x64(),
            target::NativeTarget::windows_x64(),
        ] {
            let effects = crate::x86_64_machine_effect_catalog(target, &constraints).unwrap();
            crate::validate_x86_64_machine_effect_catalog(target, &constraints, effects.clone())
                .unwrap();
            let row = effects
                .declaration_by_semantic(selected_instructions::MachineSemanticKind::ZeroBytes)
                .unwrap();
            assert_eq!(
                row.memory,
                selected_instructions::MachineMemoryEffect::ZeroBytesV1
            );
            assert_eq!(row.alternatives[0].encoded.external_operand_reads, [0, 1]);
            assert_eq!(row.alternatives[0].encoded.external_operand_writes, [2]);
        }
    }

    #[test]
    fn zero_bytes_preserves_inputs_and_rejects_every_changed_encoding_bit() {
        let physical = register_model::validate_physical_register_model(
            crate::x86_64_physical_register_model(),
        )
        .unwrap();
        let alternative = MachineAlternativeKey {
            family: MachineAlternativeFamily::ZeroBytes,
            variant: 0,
        };
        let operands =
            ["rdi", "rcx", "rax"].map(|name| physical.model().view_named(name).unwrap().id);
        // xor rax, rax; test rcx, rcx; jz +17; mov byte [rdi + rax], 0;
        // inc rax; cmp rax, rcx; jb -17
        assert_eq!(
            encode(&physical, alternative, &operands).unwrap().bytes(),
            [
                0x48, 0x31, 0xc0, 0x48, 0x85, 0xc9, 0x74, 0x11, 0x40, 0xc6, 0x84, 0x07, 0, 0, 0, 0,
                0, 0x48, 0xff, 0xc0, 0x48, 0x39, 0xc8, 0x72, 0xef,
            ]
        );
        for names in [
            ["r13", "r12", "r15"],
            ["rax", "rcx", "rdx"],
            ["rbp", "r8", "r12"],
        ] {
            let operands = names.map(|name| physical.model().view_named(name).unwrap().id);
            let encoded = encode(&physical, alternative, &operands).unwrap();
            assert_eq!(
                encode_x86_64_selected_form(
                    &physical,
                    SelectedInstructionKind::ZeroBytes,
                    alternative,
                    &operands
                )
                .unwrap(),
                encoded
            );
            validate_x86_64_selected_form_encoding(
                &physical,
                SelectedInstructionKind::ZeroBytes,
                alternative,
                &operands,
                encoded.bytes(),
            )
            .unwrap();
            assert_eq!(encoded.bytes().len(), 25);
            assert_eq!(encoded.footprint().register_reads, operands[..2]);
            assert_eq!(encoded.footprint().register_writes, operands[2..]);
            for bit in 0..encoded.bytes().len() * 8 {
                let mut changed = encoded.bytes().to_vec();
                changed[bit / 8] ^= 1 << (bit % 8);
                assert!(
                    validate(&physical, alternative, &operands, &changed).is_err(),
                    "bit {bit}"
                );
            }
            super::super::assert_byte_fragility(encoded.bytes(), |bytes| {
                validate(&physical, alternative, &operands, bytes).is_err()
            });
            for operand in 0..3 {
                let mut changed = operands;
                changed[operand] = physical.model().view_named("rsp").unwrap().id;
                assert!(encode(&physical, alternative, &changed).is_err());
            }
            for input in 0..2 {
                let mut changed = operands;
                changed[2] = operands[input];
                assert!(encode(&physical, alternative, &changed).is_err());
            }
        }
    }
}
