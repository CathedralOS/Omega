use register_model::validate_physical_register_model;
use selected_instructions::{
    MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedEffects, SelectedInstructionKind,
};

use super::{encode_aarch64_selected_form, validate_aarch64_selected_form_encoding};
use crate::aarch64_physical_register_model;

fn forms() -> [(SelectedInstructionKind, MachineAlternativeFamily); 6] {
    [
        (
            SelectedInstructionKind::ZeroExtendU8,
            MachineAlternativeFamily::ZeroExtendU8,
        ),
        (
            SelectedInstructionKind::ZeroExtendU16,
            MachineAlternativeFamily::ZeroExtendU16,
        ),
        (
            SelectedInstructionKind::ZeroExtendU32,
            MachineAlternativeFamily::ZeroExtendU32,
        ),
        (
            SelectedInstructionKind::SignExtendI8,
            MachineAlternativeFamily::SignExtendI8,
        ),
        (
            SelectedInstructionKind::SignExtendI16,
            MachineAlternativeFamily::SignExtendI16,
        ),
        (
            SelectedInstructionKind::SignExtendI32,
            MachineAlternativeFamily::SignExtendI32,
        ),
    ]
}

#[test]
fn normalization_binds_signedness_width_registers_and_full_destination_effects() {
    let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
    let names = ["x0", "x1", "x9", "x16", "x29", "x30"];
    for source in names {
        for destination in names {
            let operands = [
                physical.model().view_named(source).unwrap().id,
                physical.model().view_named(destination).unwrap().id,
            ];
            for (kind, family) in forms() {
                let key = MachineAlternativeKey { family, variant: 0 };
                let encoded =
                    encode_aarch64_selected_form(&physical, kind, key, &operands).unwrap();
                let footprint = encoded.footprint();
                assert_eq!(footprint.register_reads, vec![operands[0]]);
                assert_eq!(footprint.register_writes, vec![operands[1]]);
                assert!(!footprint.writes_nzcv);
                assert_eq!(
                    footprint.encoded,
                    MachineEncodedEffects::fallthrough_v1(vec![0], vec![1])
                );
                for (other_kind, other_family) in forms() {
                    if other_kind == kind {
                        continue;
                    }
                    let other_key = MachineAlternativeKey {
                        family: other_family,
                        variant: 0,
                    };
                    assert!(
                        validate_aarch64_selected_form_encoding(
                            &physical,
                            other_kind,
                            other_key,
                            &operands,
                            encoded.bytes()
                        )
                        .is_err(),
                        "{kind:?} must not replay as {other_kind:?}"
                    );
                    assert!(
                        validate_aarch64_selected_form_encoding(
                            &physical,
                            kind,
                            other_key,
                            &operands,
                            encoded.bytes()
                        )
                        .is_err()
                    );
                }
                for bit_offset in 0..encoded.bytes().len() * 8 {
                    let mut changed = encoded.bytes().to_vec();
                    changed[bit_offset / 8] ^= 1 << (bit_offset % 8);
                    assert!(
                        validate_aarch64_selected_form_encoding(
                            &physical, kind, key, &operands, &changed
                        )
                        .is_err(),
                        "{kind:?} accepted changed bit {bit_offset}"
                    );
                }
            }
        }
    }
}

#[test]
fn normalization_matches_clang_assembler_oracle() {
    // Apple clang integrated assembler, disassembled independently with llvm-objdump.
    let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
    let fixtures: [(
        SelectedInstructionKind,
        MachineAlternativeFamily,
        &str,
        &str,
        &[u8],
    ); 4] = [
        (
            SelectedInstructionKind::ZeroExtendU16,
            MachineAlternativeFamily::ZeroExtendU16,
            "x16",
            "x9",
            &[0x09, 0x3e, 0x40, 0xd3],
        ),
        (
            SelectedInstructionKind::SignExtendI8,
            MachineAlternativeFamily::SignExtendI8,
            "x16",
            "x9",
            &[0x09, 0x1e, 0x40, 0x93],
        ),
        (
            SelectedInstructionKind::SignExtendI16,
            MachineAlternativeFamily::SignExtendI16,
            "x16",
            "x9",
            &[0x09, 0x3e, 0x40, 0x93],
        ),
        (
            SelectedInstructionKind::SignExtendI32,
            MachineAlternativeFamily::SignExtendI32,
            "x16",
            "x9",
            &[0x09, 0x7e, 0x40, 0x93],
        ),
    ];
    for (kind, family, source, destination, expected) in fixtures {
        let operands = [
            physical.model().view_named(source).unwrap().id,
            physical.model().view_named(destination).unwrap().id,
        ];
        let key = MachineAlternativeKey { family, variant: 0 };
        assert_eq!(
            encode_aarch64_selected_form(&physical, kind, key, &operands)
                .unwrap()
                .bytes(),
            expected
        );
        validate_aarch64_selected_form_encoding(&physical, kind, key, &operands, expected).unwrap();
    }
}
