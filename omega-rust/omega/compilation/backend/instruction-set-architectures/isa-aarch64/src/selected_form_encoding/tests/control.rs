//! Control-form tests: `ret x30` custody, nonzero and less-than branch
//! encodings, fused cbnz, and widened imm26 skip forms.

use super::*;

#[test]
fn ret_x30_is_exact_and_separates_abi_result_custody_from_encoded_effects() {
    let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
    let x0 = physical.model().view_named("x0").unwrap().id;
    let x1 = physical.model().view_named("x1").unwrap().id;
    let x30 = physical.model().view_named("x30").unwrap();
    let pc = physical.model().view_named("pc").unwrap();
    let kind = SelectedInstructionKind::ReturnScalar;
    let alternative = alternative(MachineAlternativeFamily::ReturnScalar);
    let encoded = encode_aarch64_selected_form(&physical, kind, alternative, &[x0]).unwrap();

    assert_eq!(encoded.bytes(), [0xc0, 0x03, 0x5f, 0xd6]);
    assert!(encoded.footprint().register_reads.is_empty());
    assert!(encoded.footprint().register_writes.is_empty());
    assert!(
        encoded
            .footprint()
            .encoded
            .external_operand_reads
            .is_empty()
    );
    assert!(
        encoded
            .footprint()
            .encoded
            .external_operand_writes
            .is_empty()
    );
    assert_eq!(encoded.footprint().encoded.implicit_unit_uses, x30.units);
    assert_eq!(encoded.footprint().encoded.implicit_unit_defs, pc.units);
    assert_eq!(
        encoded.footprint().encoded.memory,
        MachineEncodedMemoryEffect::NoneV1
    );
    assert_eq!(
        encoded.footprint().encoded.stack,
        MachineEncodedStackEffect::UnchangedV1
    );
    assert_eq!(
        encoded.footprint().encoded.trap,
        MachineEncodedTrapBehavior::MayArchitecturalFaultV1
    );
    assert_eq!(
        encoded.footprint().encoded.control,
        MachineEncodedControlEffect::ReturnIndirectRegisterV1 { target: x30.id }
    );
    assert!(encode_aarch64_selected_form(&physical, kind, alternative, &[x1]).is_err());
    assert!(
        validate_aarch64_selected_form_encoding(
            &physical,
            kind,
            alternative,
            &[x0],
            &0xd65f_03a0_u32.to_le_bytes()
        )
        .is_err()
    );
    assert!(
        validate_aarch64_selected_form_encoding(
            &physical,
            kind,
            alternative,
            &[x0],
            &[0xc0, 0x03, 0x5f, 0xd6, 0xc0, 0x03, 0x5f, 0xd6]
        )
        .is_err()
    );
}

#[test]
fn unit_return_is_a_distinct_zero_operand_ret_x30() {
    let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
    let kind = SelectedInstructionKind::ReturnUnit;
    let return_alternative = alternative(MachineAlternativeFamily::ReturnUnit);
    let encoded = encode_aarch64_selected_form(&physical, kind, return_alternative, &[]).unwrap();

    assert_eq!(encoded.bytes(), [0xc0, 0x03, 0x5f, 0xd6]);
    assert!(encoded.footprint().register_reads.is_empty());
    assert!(encoded.footprint().register_writes.is_empty());
    assert!(matches!(
        encoded.footprint().encoded.control,
        MachineEncodedControlEffect::ReturnIndirectRegisterV1 { .. }
    ));
    assert!(
        encode_aarch64_selected_form(
            &physical,
            kind,
            alternative(MachineAlternativeFamily::ReturnScalar),
            &[]
        )
        .is_err()
    );
}

#[test]
fn nonzero_branch_has_exact_instruction_relative_imm19_and_effects() {
    let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
    let alternative = alternative(MachineAlternativeFamily::ConditionalBranchNonZero);
    for displacement in [-1_048_576, -4, 0, 4, 1_048_572] {
        let encoded = encode_aarch64_selected_conditional_branch_form(
            &physical,
            alternative,
            MachineAlternativeFamily::ConditionalBranchNonZero,
            Aarch64ConditionalBranchForm::CanonicalWordV1,
            displacement,
        )
        .unwrap();
        assert_eq!(encoded.bytes().len(), 4);
        assert_eq!(encoded.bytes()[0] & 0x1f, 1);
        assert!(encoded.footprint().register_reads.is_empty());
        assert!(encoded.footprint().register_writes.is_empty());
        assert_eq!(
            encoded.footprint().encoded.control,
            MachineEncodedControlEffect::ConditionalRelativeBranchV1
        );
    }
    for displacement in [-1_048_580, 1_048_576, 2] {
        assert!(
            encode_aarch64_selected_conditional_branch_form(
                &physical,
                alternative,
                MachineAlternativeFamily::ConditionalBranchNonZero,
                Aarch64ConditionalBranchForm::CanonicalWordV1,
                displacement
            )
            .is_err()
        );
    }
    assert!(
        validate_aarch64_selected_conditional_branch_form(
            &physical,
            alternative,
            MachineAlternativeFamily::ConditionalBranchNonZero,
            Aarch64ConditionalBranchForm::CanonicalWordV1,
            0,
            &0x5400_0000_u32.to_le_bytes()
        )
        .is_err()
    );
    assert!(
        validate_aarch64_selected_conditional_branch_form(
            &physical,
            alternative,
            MachineAlternativeFamily::ConditionalBranchNonZero,
            Aarch64ConditionalBranchForm::CanonicalWordV1,
            0,
            &[1, 0, 0, 0, 0]
        )
        .is_err()
    );
}

#[test]
fn u64_less_than_branch_is_exact_b_lo_imm19_with_flag_control_effects() {
    let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
    let nzcv = physical.model().view_named("nzcv").unwrap();
    let pc = physical.model().view_named("pc").unwrap();
    let alternative = alternative(MachineAlternativeFamily::ConditionalBranchU64LessThan);
    for (displacement, expected) in [
        (-4, [0xe3, 0xff, 0xff, 0x54]),
        (0, [0x03, 0x00, 0x00, 0x54]),
        (4, [0x23, 0x00, 0x00, 0x54]),
    ] {
        let encoded = encode_aarch64_selected_conditional_branch_form(
            &physical,
            alternative,
            MachineAlternativeFamily::ConditionalBranchU64LessThan,
            Aarch64ConditionalBranchForm::CanonicalWordV1,
            displacement,
        )
        .unwrap();
        assert_eq!(encoded.bytes(), expected);
        assert!(encoded.footprint().register_reads.is_empty());
        assert!(encoded.footprint().register_writes.is_empty());
        assert_eq!(
            encoded.footprint().encoded.control,
            MachineEncodedControlEffect::ConditionalRelativeBranchV1
        );
        assert!(nzcv.units.iter().all(|unit| {
            encoded
                .footprint()
                .encoded
                .implicit_unit_uses
                .contains(unit)
        }));
        assert!(pc.units.iter().all(|unit| {
            encoded
                .footprint()
                .encoded
                .implicit_unit_uses
                .contains(unit)
        }));
        assert_eq!(encoded.footprint().encoded.implicit_unit_defs, pc.units);
    }
    for displacement in [-1_048_576, 1_048_572] {
        assert!(
            encode_aarch64_selected_conditional_branch_form(
                &physical,
                alternative,
                MachineAlternativeFamily::ConditionalBranchU64LessThan,
                Aarch64ConditionalBranchForm::CanonicalWordV1,
                displacement,
            )
            .is_ok()
        );
    }
    for displacement in [-1_048_580, 1_048_576, 2] {
        assert!(
            encode_aarch64_selected_conditional_branch_form(
                &physical,
                alternative,
                MachineAlternativeFamily::ConditionalBranchU64LessThan,
                Aarch64ConditionalBranchForm::CanonicalWordV1,
                displacement,
            )
            .is_err()
        );
    }
    for bytes in [
        0x5400_0002_u32.to_le_bytes(),
        0x5400_0009_u32.to_le_bytes(),
        0x1400_0003_u32.to_le_bytes(),
    ] {
        assert_eq!(
            validate_aarch64_selected_conditional_branch_form(
                &physical,
                alternative,
                MachineAlternativeFamily::ConditionalBranchU64LessThan,
                Aarch64ConditionalBranchForm::CanonicalWordV1,
                0,
                &bytes,
            ),
            Err(Aarch64SelectedFormEncodingError::MalformedEncoding)
        );
    }
    assert_eq!(
        validate_aarch64_selected_conditional_branch_form(
            &physical,
            alternative,
            MachineAlternativeFamily::ConditionalBranchU64LessThan,
            Aarch64ConditionalBranchForm::CanonicalWordV1,
            0,
            &0x5400_0023_u32.to_le_bytes(),
        ),
        Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch)
    );
}

#[test]
fn i64_less_than_branch_is_exact_b_lt_imm19() {
    let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
    let alternative = alternative(MachineAlternativeFamily::ConditionalBranchI64LessThan);
    for (displacement, expected) in [
        (-4, [0xeb, 0xff, 0xff, 0x54]),
        (0, [0x0b, 0x00, 0x00, 0x54]),
        (4, [0x2b, 0x00, 0x00, 0x54]),
    ] {
        let encoded = encode_aarch64_selected_conditional_branch_form(
            &physical,
            alternative,
            MachineAlternativeFamily::ConditionalBranchI64LessThan,
            Aarch64ConditionalBranchForm::CanonicalWordV1,
            displacement,
        )
        .unwrap();
        assert_eq!(encoded.bytes(), expected);
        assert_eq!(
            encoded.footprint().encoded.control,
            MachineEncodedControlEffect::ConditionalRelativeBranchV1
        );
    }
    assert_eq!(
        validate_aarch64_selected_conditional_branch_form(
            &physical,
            alternative,
            MachineAlternativeFamily::ConditionalBranchI64LessThan,
            Aarch64ConditionalBranchForm::CanonicalWordV1,
            0,
            &0x5400_0003_u32.to_le_bytes(),
        ),
        Err(Aarch64SelectedFormEncodingError::MalformedEncoding)
    );
}

#[test]
fn fused_cbnz_is_exact_rejects_nearby_opcodes_and_does_not_read_nzcv() {
    let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
    let x0 = physical.model().view_named("x0").unwrap();
    let x30 = physical.model().view_named("x30").unwrap();
    let pc = physical.model().view_named("pc").unwrap();
    let nzcv = physical.model().view_named("nzcv").unwrap();

    for (source, register) in [(x0.id, 0_u8), (x30.id, 30_u8)] {
        for displacement in [-1_048_576, -4, 0, 4, 1_048_572] {
            let encoded = encode_aarch64_fused_compare_i64_zero_branch_nonzero_to_cbnz_form(
                &physical,
                source,
                displacement,
            )
            .unwrap();
            assert_eq!(encoded.bytes().len(), 4);
            assert_eq!(encoded.bytes()[0] & 0x1f, register);
            assert_eq!(encoded.footprint().register_reads, [source]);
            assert!(encoded.footprint().register_writes.is_empty());
            assert!(!encoded.footprint().writes_nzcv);
            assert!(
                encoded
                    .footprint()
                    .encoded
                    .external_operand_reads
                    .is_empty()
            );
            assert_eq!(encoded.footprint().encoded.implicit_unit_uses, pc.units);
            assert_eq!(encoded.footprint().encoded.implicit_unit_defs, pc.units);
            assert!(
                encoded
                    .footprint()
                    .encoded
                    .implicit_unit_uses
                    .iter()
                    .all(|unit| !nzcv.units.contains(unit))
            );
        }
    }

    for displacement in [-1_048_580, 1_048_576, 2] {
        assert!(
            encode_aarch64_fused_compare_i64_zero_branch_nonzero_to_cbnz_form(
                &physical,
                x0.id,
                displacement,
            )
            .is_err()
        );
    }
    for word in [
        0xb400_0000_u32, // 64-bit CBZ
        0x3500_0000_u32, // 32-bit CBNZ
        0xb500_001e_u32, // wrong source register
    ] {
        assert!(
            validate_aarch64_fused_compare_i64_zero_branch_nonzero_to_cbnz_form(
                &physical,
                x0.id,
                0,
                &word.to_le_bytes(),
            )
            .is_err()
        );
    }
    assert!(
        validate_aarch64_fused_compare_i64_zero_branch_nonzero_to_cbnz_form(
            &physical,
            x0.id,
            0,
            &[0, 0, 0, 0, 0],
        )
        .is_err()
    );
}

#[test]
fn widened_branch_forms_encode_inverted_skip_plus_unconditional_imm26() {
    let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
    let cases: [(MachineAlternativeFamily, u32); 3] = [
        (MachineAlternativeFamily::ConditionalBranchNonZero, 0x0),
        (MachineAlternativeFamily::ConditionalBranchU64LessThan, 0x2),
        (MachineAlternativeFamily::ConditionalBranchI64LessThan, 0xa),
    ];
    // Displacements beyond `B.cond` imm19 (±1 MiB) but inside `B` imm26
    // (±128 MiB) encode; the unconditional word's own displacement is the
    // row displacement minus one word.
    for (family, inverted_condition) in cases {
        let alternative = alternative(family);
        for row_displacement in [-1_048_580, -4, 4, 1_048_576, 134_217_724, 134_217_728] {
            let encoded = encode_aarch64_selected_conditional_branch_form(
                &physical,
                alternative,
                family,
                Aarch64ConditionalBranchForm::WidenedPairV1,
                row_displacement,
            )
            .unwrap();
            assert_eq!(encoded.bytes().len(), 8);
            let skip = u32::from_le_bytes(encoded.bytes()[0..4].try_into().unwrap());
            assert_eq!(skip, 0x5400_0000 | (2 << 5) | inverted_condition);
            let target = u32::from_le_bytes(encoded.bytes()[4..8].try_into().unwrap());
            assert_eq!(target & 0xfc00_0000, 0x1400_0000);
            let decoded = i64::from(((target & 0x03ff_ffff) << 6) as i32 >> 6) * 4;
            assert_eq!(decoded, row_displacement - 4);
            assert_eq!(
                encoded.footprint().encoded.control,
                MachineEncodedControlEffect::ConditionalRelativeBranchV1
            );
        }
        for displacement in [-134_217_732, 134_217_732, 2] {
            assert!(
                encode_aarch64_selected_conditional_branch_form(
                    &physical,
                    alternative,
                    family,
                    Aarch64ConditionalBranchForm::WidenedPairV1,
                    displacement,
                )
                .is_err()
            );
        }
    }
}

#[test]
fn widened_branch_forms_decode_and_reject_corruption() {
    let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
    let cases: [(MachineAlternativeFamily, u32); 3] = [
        (MachineAlternativeFamily::ConditionalBranchNonZero, 0x0),
        (MachineAlternativeFamily::ConditionalBranchU64LessThan, 0x2),
        (MachineAlternativeFamily::ConditionalBranchI64LessThan, 0xa),
    ];
    for (family, inverted_condition) in cases {
        let alternative = alternative(family);
        let row_displacement = 2_000_000;
        let encoded = encode_aarch64_selected_conditional_branch_form(
            &physical,
            alternative,
            family,
            Aarch64ConditionalBranchForm::WidenedPairV1,
            row_displacement,
        )
        .unwrap();
        let round_trip = validate_aarch64_selected_conditional_branch_form(
            &physical,
            alternative,
            family,
            Aarch64ConditionalBranchForm::WidenedPairV1,
            row_displacement,
            encoded.bytes(),
        )
        .unwrap();
        assert_eq!(round_trip.bytes(), encoded.bytes());
        // The canonical four-byte form is not a widened row.
        assert_eq!(
            validate_aarch64_selected_conditional_branch_form(
                &physical,
                alternative,
                family,
                Aarch64ConditionalBranchForm::WidenedPairV1,
                0,
                &encoded.bytes()[0..4],
            ),
            Err(Aarch64SelectedFormEncodingError::MalformedEncoding)
        );
        // A skip word carrying the non-inverted condition is not this form.
        let wrong_skip = 0x5400_0000_u32 | (2 << 5) | (inverted_condition ^ 0xf);
        let mut corrupt = encoded.bytes().to_vec();
        crate::codec_bytes::patch_encoded(&mut corrupt, 0, wrong_skip.to_le_bytes());
        assert_eq!(
            validate_aarch64_selected_conditional_branch_form(
                &physical,
                alternative,
                family,
                Aarch64ConditionalBranchForm::WidenedPairV1,
                row_displacement,
                &corrupt,
            ),
            Err(Aarch64SelectedFormEncodingError::MalformedEncoding)
        );
        // A second word that is not an unconditional `B` rejects.
        let mut corrupt = encoded.bytes().to_vec();
        crate::codec_bytes::patch_encoded(&mut corrupt, 4, 0x5400_000b_u32.to_le_bytes());
        assert_eq!(
            validate_aarch64_selected_conditional_branch_form(
                &physical,
                alternative,
                family,
                Aarch64ConditionalBranchForm::WidenedPairV1,
                row_displacement,
                &corrupt,
            ),
            Err(Aarch64SelectedFormEncodingError::MalformedEncoding)
        );
        // Valid words encoding a different displacement reject.
        let other = encode_aarch64_selected_conditional_branch_form(
            &physical,
            alternative,
            family,
            Aarch64ConditionalBranchForm::WidenedPairV1,
            row_displacement - 8,
        )
        .unwrap();
        assert_eq!(
            validate_aarch64_selected_conditional_branch_form(
                &physical,
                alternative,
                family,
                Aarch64ConditionalBranchForm::WidenedPairV1,
                row_displacement,
                other.bytes(),
            ),
            Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch)
        );
        // A foreign alternative key rejects.
        let foreign = self::alternative(MachineAlternativeFamily::Jump);
        assert_eq!(
            encode_aarch64_selected_conditional_branch_form(
                &physical,
                foreign,
                family,
                Aarch64ConditionalBranchForm::WidenedPairV1,
                row_displacement,
            )
            .map(|encoding| encoding.bytes().len()),
            Err(Aarch64SelectedFormEncodingError::AlternativeMismatch)
        );
    }
}
