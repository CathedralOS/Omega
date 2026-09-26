//! Boolean conditions consume flags and establish a complete canonical register value.
use super::{encode_aarch64_selected_form, validate_aarch64_selected_form_encoding};
use crate::aarch64_physical_register_model;
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
    let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
    for name in ["x0", "x1", "x9", "x16", "x29", "x30"] {
        let destination = physical.model().view_named(name).unwrap().id;
        for (kind, family) in forms() {
            let key = MachineAlternativeKey { family, variant: 0 };
            let encoded =
                encode_aarch64_selected_form(&physical, kind, key, &[destination]).unwrap();
            validate_aarch64_selected_form_encoding(
                &physical,
                kind,
                key,
                &[destination],
                encoded.bytes(),
            )
            .unwrap();
            assert_eq!(encoded.bytes().len(), 4);
            let footprint = encoded.footprint();
            assert!(footprint.register_reads.is_empty());
            assert_eq!(footprint.register_writes, vec![destination]);
            assert!(!footprint.writes_nzcv);
            let mut expected = MachineEncodedEffects::fallthrough_v1(vec![], vec![0]);
            expected.implicit_unit_uses =
                physical.model().view_named("nzcv").unwrap().units.clone();
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
                    validate_aarch64_selected_form_encoding(
                        &physical,
                        other_kind,
                        other_key,
                        &[destination],
                        encoded.bytes()
                    )
                    .is_err()
                );
                assert!(
                    validate_aarch64_selected_form_encoding(
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
                    validate_aarch64_selected_form_encoding(
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
                    validate_aarch64_selected_form_encoding(
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
                validate_aarch64_selected_form_encoding(
                    &physical,
                    kind,
                    key,
                    &[destination],
                    &extra
                )
                .is_err()
            );
            for invalid in ["sp", "xzr", "w0", "nzcv"] {
                let view = physical.model().view_named(invalid).unwrap().id;
                assert!(
                    encode_aarch64_selected_form(&physical, kind, key, &[view]).is_err(),
                    "invalid destination {invalid}"
                );
            }
        }
    }
}

#[test]
fn cset_uses_inverse_condition_and_two_zero_register_inputs() {
    let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
    let destination = physical.model().view_named("x0").unwrap().id;
    // CSET X0, EQ/LO/LT/LS/LE aliases CSINC with inverted condition.
    for ((kind, family), word) in forms().into_iter().zip([
        0x9a9f_17e0_u32,
        0x9a9f_27e0,
        0x9a9f_a7e0,
        0x9a9f_87e0,
        0x9a9f_c7e0,
    ]) {
        let key = MachineAlternativeKey { family, variant: 0 };
        assert_eq!(
            encode_aarch64_selected_form(&physical, kind, key, &[destination])
                .unwrap()
                .bytes(),
            word.to_le_bytes()
        );
    }
}
