//! Boolean conditions consume flags and establish a complete canonical register value.
use super::{encode_x86_64_selected_form, validate_x86_64_selected_form_encoding};
use crate::x86_64_physical_register_model;
use register_model::validate_physical_register_model;
use selected_instructions::{
    MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedEffects, SelectedInstructionKind,
};

fn forms() -> [(SelectedInstructionKind, MachineAlternativeFamily); 5] {
    [
        (
            SelectedInstructionKind::MaterializeBooleanEqual,
            MachineAlternativeFamily::MaterializeBooleanEqual,
        ),
        (
            SelectedInstructionKind::MaterializeBooleanU64LessThan,
            MachineAlternativeFamily::MaterializeBooleanU64LessThan,
        ),
        (
            SelectedInstructionKind::MaterializeBooleanI64LessThan,
            MachineAlternativeFamily::MaterializeBooleanI64LessThan,
        ),
        (
            SelectedInstructionKind::MaterializeBooleanU64LessOrEqual,
            MachineAlternativeFamily::MaterializeBooleanU64LessOrEqual,
        ),
        (
            SelectedInstructionKind::MaterializeBooleanI64LessOrEqual,
            MachineAlternativeFamily::MaterializeBooleanI64LessOrEqual,
        ),
    ]
}

#[test]
fn conditions_bind_registers_full_width_and_preserved_flags() {
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    for name in ["rax", "rcx", "rbp", "rsi", "rdi", "r8", "r9", "r15"] {
        let destination = physical.model().view_named(name).unwrap().id;
        for (kind, family) in forms() {
            let key = MachineAlternativeKey { family, variant: 0 };
            let encoded =
                encode_x86_64_selected_form(&physical, kind, key, &[destination]).unwrap();
            validate_x86_64_selected_form_encoding(
                &physical,
                kind,
                key,
                &[destination],
                encoded.bytes(),
            )
            .unwrap();
            assert_eq!(encoded.bytes().len(), 8);
            let footprint = encoded.footprint();
            assert!(footprint.register_reads.is_empty());
            assert_eq!(footprint.register_writes, vec![destination]);
            assert!(!footprint.writes_rflags);
            let mut expected = MachineEncodedEffects::fallthrough_v1(vec![], vec![0]);
            expected.implicit_unit_uses =
                physical.model().view_named("rflags").unwrap().units.clone();
            assert_eq!(footprint.encoded, expected);
            for (other_kind, other_family) in forms() {
                if kind == other_kind {
                    continue;
                }
                let other_key = MachineAlternativeKey {
                    family: other_family,
                    variant: 0,
                };
                assert!(
                    validate_x86_64_selected_form_encoding(
                        &physical,
                        other_kind,
                        other_key,
                        &[destination],
                        encoded.bytes()
                    )
                    .is_err()
                );
                assert!(
                    validate_x86_64_selected_form_encoding(
                        &physical,
                        kind,
                        other_key,
                        &[destination],
                        encoded.bytes()
                    )
                    .is_err()
                );
            }
            // Includes condition inversion, destination bits, width, register-mode
            // bits, and (on x86) both REX extensions and MOVZX's source/destination.
            for bit in 0..encoded.bytes().len() * 8 {
                let mut changed = encoded.bytes().to_vec();
                changed[bit / 8] ^= 1 << (bit % 8);
                assert!(
                    validate_x86_64_selected_form_encoding(
                        &physical,
                        kind,
                        key,
                        &[destination],
                        &changed
                    )
                    .is_err(),
                    "{kind:?} {name} accepted bit {bit}"
                );
            }
            for length in 0..encoded.bytes().len() {
                assert!(
                    validate_x86_64_selected_form_encoding(
                        &physical,
                        kind,
                        key,
                        &[destination],
                        &encoded.bytes()[..length]
                    )
                    .is_err()
                );
            }
            let mut extra = encoded.bytes().to_vec();
            extra.extend_from_slice(encoded.bytes());
            assert!(
                validate_x86_64_selected_form_encoding(
                    &physical,
                    kind,
                    key,
                    &[destination],
                    &extra
                )
                .is_err()
            );
            for invalid in ["rsp", "eax", "ah", "xmm0", "rflags"] {
                let view = physical.model().view_named(invalid).unwrap().id;
                assert!(
                    encode_x86_64_selected_form(&physical, kind, key, &[view]).is_err(),
                    "invalid destination {invalid}"
                );
            }
        }
    }
}

#[test]
fn condition_opcodes_and_rex_low_byte_normalization_are_exact() {
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    // SETE, SETB, SETL, SETBE, SETLE; the second instruction is MOVZX r32,r8.
    for ((kind, family), opcode) in forms().into_iter().zip([0x94, 0x92, 0x9c, 0x96, 0x9e]) {
        let key = MachineAlternativeKey { family, variant: 0 };
        for (name, expected) in [
            ("rsi", [0x40, 0x0f, opcode, 0xc6, 0x40, 0x0f, 0xb6, 0xf6]),
            ("r15", [0x41, 0x0f, opcode, 0xc7, 0x45, 0x0f, 0xb6, 0xff]),
        ] {
            let destination = physical.model().view_named(name).unwrap().id;
            assert_eq!(
                encode_x86_64_selected_form(&physical, kind, key, &[destination])
                    .unwrap()
                    .bytes(),
                expected
            );
            let mut missing_rex = expected.to_vec();
            missing_rex.remove(0);
            assert!(
                validate_x86_64_selected_form_encoding(
                    &physical,
                    kind,
                    key,
                    &[destination],
                    &missing_rex
                )
                .is_err()
            );
        }
    }
}
