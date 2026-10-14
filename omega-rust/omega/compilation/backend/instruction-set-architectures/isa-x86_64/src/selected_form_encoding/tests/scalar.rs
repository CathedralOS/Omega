//! Scalar-encoding form tests: explicit call refusal, zero-extend and
//! multiply bindings, compare canonical footprints, alias partitions,
//! bitwise and immediate-LEA encoding, and mutation rejection.

use super::*;

#[test]
fn scalar_call_is_explicitly_refused_before_encoding() {
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    assert_eq!(
        encode_x86_64_selected_form(
            &physical,
            SelectedInstructionKind::CallScalar {
                target: selected_instructions::SelectedCallTarget::Machine(
                    MachineId::new(1).unwrap()
                ),
            },
            alternative(MachineAlternativeFamily::ReturnUnit, 0),
            &[],
        ),
        Err(X86_64SelectedFormEncodingError::LayoutDependentForm),
    );
}

#[test]
fn zero_extend_u8_binds_width_registers_and_is_not_a_copy() {
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    for source in ["rax", "rcx", "rsi", "rdi", "r8", "r15"] {
        for destination in ["rax", "rcx", "rsi", "rdi", "r8", "r15"] {
            let operands = [
                physical.model().view_named(source).unwrap().id,
                physical.model().view_named(destination).unwrap().id,
            ];
            let kind = SelectedInstructionKind::ZeroExtendU8;
            let key = alternative(MachineAlternativeFamily::ZeroExtendU8, 0);
            let encoded = encode_x86_64_selected_form(&physical, kind, key, &operands).unwrap();
            assert_eq!(encoded.bytes().len(), 4);
            validate_x86_64_selected_form_encoding(
                &physical,
                kind,
                key,
                &operands,
                encoded.bytes(),
            )
            .unwrap();
            let copy = encode_x86_64_selected_form(
                &physical,
                SelectedInstructionKind::CopyI64,
                alternative(MachineAlternativeFamily::CopyI64, 0),
                &operands,
            )
            .unwrap();
            assert!(
                validate_x86_64_selected_form_encoding(
                    &physical,
                    kind,
                    key,
                    &operands,
                    copy.bytes()
                )
                .is_err()
            );
            assert_every_byte_change_rejected(
                &physical,
                kind,
                key,
                &operands,
                encoded.bytes(),
                "changed",
            );
        }
    }
}

#[test]
fn zero_extend_u32_binds_width_registers_and_is_not_a_copy() {
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    for source in ["rax", "rcx", "rsi", "rdi", "r8", "r15"] {
        for destination in ["rax", "rcx", "rsi", "rdi", "r8", "r15"] {
            let operands = [
                physical.model().view_named(source).unwrap().id,
                physical.model().view_named(destination).unwrap().id,
            ];
            let kind = SelectedInstructionKind::ZeroExtendU32;
            let key = alternative(MachineAlternativeFamily::ZeroExtendU32, 0);
            let encoded = encode_x86_64_selected_form(&physical, kind, key, &operands).unwrap();
            assert_eq!(encoded.bytes().len(), 3);
            // A 32-bit MOV writes the full parent register with zero upper bits.
            assert_eq!(encoded.bytes()[0] & 0xf8, 0x40);
            assert_eq!(encoded.bytes()[1], 0x89);
            validate_x86_64_selected_form_encoding(
                &physical,
                kind,
                key,
                &operands,
                encoded.bytes(),
            )
            .unwrap();
            let narrow = encode_x86_64_selected_form(
                &physical,
                SelectedInstructionKind::ZeroExtendU8,
                alternative(MachineAlternativeFamily::ZeroExtendU8, 0),
                &operands,
            )
            .unwrap();
            assert!(
                validate_x86_64_selected_form_encoding(
                    &physical,
                    kind,
                    key,
                    &operands,
                    narrow.bytes()
                )
                .is_err()
            );
            let copy = encode_x86_64_selected_form(
                &physical,
                SelectedInstructionKind::CopyI64,
                alternative(MachineAlternativeFamily::CopyI64, 0),
                &operands,
            )
            .unwrap();
            assert!(
                validate_x86_64_selected_form_encoding(
                    &physical,
                    kind,
                    key,
                    &operands,
                    copy.bytes()
                )
                .is_err()
            );
            assert_every_byte_change_rejected(
                &physical,
                kind,
                key,
                &operands,
                encoded.bytes(),
                "changed",
            );
        }
    }
}

#[test]
fn r12_is_a_valid_rex_extended_sib_index() {
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    let r12 = physical.model().view_named("r12").unwrap().id;
    let rax = physical.model().view_named("rax").unwrap().id;
    let encoded = encode_x86_64_selected_form(
        &physical,
        exact_add(),
        alternative(MachineAlternativeFamily::ExactAddI64, 0),
        &[r12, r12, rax],
    )
    .unwrap();
    assert_eq!(encoded.bytes(), [0x4b, 0x8d, 0x04, 0x24]);
}

#[test]
fn compare_i64_is_canonical_register_cmp_with_exact_footprint() {
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    let rax = physical.model().view_named("rax").unwrap().id;
    let rbx = physical.model().view_named("rbx").unwrap().id;
    let r8 = physical.model().view_named("r8").unwrap().id;
    let r9 = physical.model().view_named("r9").unwrap().id;
    let alternative = alternative(MachineAlternativeFamily::CompareI64, 0);
    for (operands, expected) in [
        ([rax, rbx], [0x48, 0x39, 0xd8]),
        ([r8, r9], [0x4d, 0x39, 0xc8]),
        ([rax, rax], [0x48, 0x39, 0xc0]),
    ] {
        let encoded = encode_x86_64_selected_form(
            &physical,
            SelectedInstructionKind::CompareI64,
            alternative,
            &operands,
        )
        .unwrap();
        assert_eq!(encoded.bytes(), expected);
        assert_eq!(encoded.footprint().register_reads, operands);
        assert!(encoded.footprint().register_writes.is_empty());
        assert!(encoded.footprint().writes_rflags);
        assert_eq!(encoded.footprint().encoded.external_operand_reads, [0, 1]);
    }
}

#[test]
fn compare_i64_immediate_is_canonical_cmp_imm32_with_exact_footprint() {
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    let rax = physical.model().view_named("rax").unwrap().id;
    let r12 = physical.model().view_named("r12").unwrap().id;
    let alternative = alternative(MachineAlternativeFamily::CompareI64Immediate, 0);
    for (operand, immediate, expected) in [
        (rax, 0, [0x48, 0x81, 0xf8, 0x00, 0x00, 0x00, 0x00]),
        (rax, 4095, [0x48, 0x81, 0xf8, 0xff, 0x0f, 0x00, 0x00]),
        (r12, 4095, [0x49, 0x81, 0xfc, 0xff, 0x0f, 0x00, 0x00]),
    ] {
        let kind = SelectedInstructionKind::CompareI64Immediate {
            immediate: IntegerValue::Unsigned(immediate),
        };
        let encoded =
            encode_x86_64_selected_form(&physical, kind, alternative, &[operand]).unwrap();
        assert_eq!(encoded.bytes(), expected);
        assert_eq!(encoded.footprint().register_reads, [operand]);
        assert!(encoded.footprint().register_writes.is_empty());
        assert!(encoded.footprint().writes_rflags);
        assert_eq!(encoded.footprint().encoded.external_operand_reads, [0]);
        assert!(
            encoded
                .footprint()
                .encoded
                .external_operand_writes
                .is_empty()
        );
        assert!(
            validate_x86_64_selected_form_encoding(
                &physical,
                kind,
                alternative,
                &[operand],
                encoded.bytes()
            )
            .is_ok()
        );
        assert_every_byte_change_rejected(
            &physical,
            kind,
            alternative,
            &[operand],
            encoded.bytes(),
            "corrupted",
        );
    }
    for immediate in [IntegerValue::Unsigned(4096), IntegerValue::Signed(-1)] {
        let kind = SelectedInstructionKind::CompareI64Immediate { immediate };
        assert!(encode_x86_64_selected_form(&physical, kind, alternative, &[rax]).is_err());
    }
}

#[test]
fn scalar_sizes_and_subtraction_alias_partitions_are_exact() {
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    let views = ["rax", "rbx", "rcx"].map(|name| physical.model().view_named(name).unwrap().id);
    let materialize = encode_x86_64_selected_form(
        &physical,
        SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Signed(-1),
        },
        alternative(MachineAlternativeFamily::MaterializeI64, 0),
        &views[..1],
    )
    .unwrap();
    assert_eq!(materialize.bytes().len(), 10);
    for (homes, variant, size) in [
        ([views[0], views[0], views[0]], 0, 3),
        ([views[0], views[1], views[0]], 1, 3),
        ([views[0], views[1], views[1]], 2, 6),
        ([views[0], views[1], views[2]], 3, 6),
    ] {
        let kind = SelectedInstructionKind::ExactSubtractI64 {
            obligation: ObligationId::new(2).unwrap(),
            accepted_fact: AcceptedObligationFactIdentity::from_bytes([4; 32]),
        };
        let encoded = encode_x86_64_selected_form(
            &physical,
            kind,
            alternative(MachineAlternativeFamily::ExactSubtractI64, variant),
            &homes,
        )
        .unwrap();
        assert_eq!(encoded.bytes().len(), size);
        let mut corrupted = encoded.bytes().to_vec();
        corrupted[1] ^= 1;
        assert!(
            validate_x86_64_selected_form_encoding(
                &physical,
                kind,
                alternative(MachineAlternativeFamily::ExactSubtractI64, variant),
                &homes,
                &corrupted,
            )
            .is_err()
        );
    }
}

#[test]
fn exact_multiply_alias_partitions_encode_and_decode() {
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    let views = ["rax", "rbx", "rcx"].map(|name| physical.model().view_named(name).unwrap().id);
    let kind = SelectedInstructionKind::ExactMultiplyI64 {
        obligation: ObligationId::new(2).unwrap(),
        accepted_fact: AcceptedObligationFactIdentity::from_bytes([4; 32]),
    };
    for (homes, variant, size) in [
        ([views[0], views[0], views[0]], 0, 4),
        ([views[0], views[1], views[0]], 1, 4),
        ([views[0], views[1], views[1]], 2, 4),
        ([views[0], views[1], views[2]], 3, 7),
    ] {
        let key = alternative(MachineAlternativeFamily::ExactMultiplyI64, variant);
        let encoded = encode_x86_64_selected_form(&physical, kind, key, &homes).unwrap();
        assert_eq!(encoded.bytes().len(), size);
        let decoded =
            validate_x86_64_selected_form_encoding(&physical, kind, key, &homes, encoded.bytes())
                .unwrap();
        // Even the fully-aliased `imul r, r` reads both logical operands: the
        // multiply consumes its in-place destination input.
        assert_eq!(decoded.footprint().encoded.external_operand_reads, [0, 1]);
        assert_eq!(decoded.footprint().encoded.external_operand_writes, [2]);
        assert!(
            !decoded
                .footprint()
                .encoded
                .implicit_unit_clobbers
                .is_empty(),
            "imul clobbers rflags"
        );
        assert_every_byte_change_rejected(&physical, kind, key, &homes, encoded.bytes(), "changed");
    }
    // A mismatched variant cannot validate the alias partition it skips.
    let homes = [views[0], views[1], views[2]];
    let encoded = encode_x86_64_selected_form(
        &physical,
        kind,
        alternative(MachineAlternativeFamily::ExactMultiplyI64, 3),
        &homes,
    )
    .unwrap();
    assert!(
        validate_x86_64_selected_form_encoding(
            &physical,
            kind,
            alternative(MachineAlternativeFamily::ExactMultiplyI64, 0),
            &homes,
            encoded.bytes(),
        )
        .is_err()
    );
}

#[test]
fn wrapping_add_preserves_aliases_and_rejects_exact_family_or_changed_bytes() {
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    let views = ["rax", "r9", "r12"].map(|name| physical.model().view_named(name).unwrap().id);
    let kind = SelectedInstructionKind::WrappingAddI64;
    let key = alternative(MachineAlternativeFamily::WrappingAddI64, 0);
    for left in views {
        for right in views {
            for output in views {
                let operands = [left, right, output];
                let encoded = encode_x86_64_selected_form(&physical, kind, key, &operands).unwrap();
                validate_x86_64_selected_form_encoding(
                    &physical,
                    kind,
                    key,
                    &operands,
                    encoded.bytes(),
                )
                .unwrap();
                assert!(!encoded.footprint().writes_rflags);
                assert_eq!(encoded.footprint().encoded.external_operand_reads, [0, 1]);
                assert_eq!(encoded.footprint().encoded.external_operand_writes, [2]);
                let mut changed = encoded.bytes().to_vec();
                changed[1] ^= 1;
                assert!(
                    validate_x86_64_selected_form_encoding(
                        &physical, kind, key, &operands, &changed
                    )
                    .is_err()
                );
                assert!(
                    encode_x86_64_selected_form(
                        &physical,
                        kind,
                        alternative(MachineAlternativeFamily::ExactAddI64, 0),
                        &operands
                    )
                    .is_err()
                );
                assert!(
                    validate_x86_64_selected_form_encoding(
                        &physical,
                        kind,
                        alternative(MachineAlternativeFamily::ExactAddI64, 0),
                        &operands,
                        encoded.bytes()
                    )
                    .is_err()
                );
            }
        }
    }
}

#[test]
fn bitwise_operations_preserve_aliases_and_reject_opcode_corruption() {
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    let views = ["rax", "r9", "r12"].map(|name| physical.model().view_named(name).unwrap().id);
    for (kind, family, expected_opcode) in [
        (
            SelectedInstructionKind::BitwiseAndI64,
            MachineAlternativeFamily::BitwiseAndI64,
            0x21,
        ),
        (
            SelectedInstructionKind::BitwiseXorI64,
            MachineAlternativeFamily::BitwiseXorI64,
            0x31,
        ),
    ] {
        let alternative = alternative(family, 0);
        for left in views {
            for right in views {
                for output in views {
                    let operands = [left, right, output];
                    let encoded =
                        encode_x86_64_selected_form(&physical, kind, alternative, &operands)
                            .unwrap();
                    assert_eq!(
                        encoded.bytes().len(),
                        if output == left || output == right {
                            3
                        } else {
                            6
                        }
                    );
                    assert!(encoded.footprint().writes_rflags);
                    assert_eq!(encoded.footprint().encoded.external_operand_reads, [0, 1]);
                    assert_eq!(encoded.footprint().encoded.external_operand_writes, [2]);
                    let mut corrupted = encoded.bytes().to_vec();
                    let opcode = corrupted.len() - 2;
                    assert_eq!(corrupted[opcode], expected_opcode);
                    // Another valid bitwise opcode must not satisfy this operation.
                    corrupted[opcode] = if expected_opcode == 0x21 { 0x31 } else { 0x21 };
                    assert!(
                        validate_x86_64_selected_form_encoding(
                            &physical,
                            kind,
                            alternative,
                            &operands,
                            &corrupted
                        )
                        .is_err()
                    );
                    corrupted = encoded.bytes().to_vec();
                    let register_byte = corrupted.len() - 1;
                    corrupted[register_byte] ^= 1;
                    assert!(
                        validate_x86_64_selected_form_encoding(
                            &physical,
                            kind,
                            alternative,
                            &operands,
                            &corrupted
                        )
                        .is_err()
                    );
                }
            }
        }
    }
}

#[test]
fn immediate_lea_uses_declared_size_extremes() {
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    let rax = physical.model().view_named("rax").unwrap().id;
    let r12 = physical.model().view_named("r12").unwrap().id;
    let fact = AcceptedObligationFactIdentity::from_bytes([5; 32]);
    for (base, immediate, size) in [(rax, 0, 4), (rax, 4095, 7), (r12, 0, 5), (r12, 4095, 8)] {
        let kind = SelectedInstructionKind::ExactAddI64Immediate {
            immediate: IntegerValue::Unsigned(immediate),
            obligation: ObligationId::new(3).unwrap(),
            accepted_fact: fact,
        };
        let encoded = encode_x86_64_selected_form(
            &physical,
            kind,
            alternative(MachineAlternativeFamily::ExactAddI64Immediate, 0),
            &[base, rax],
        )
        .unwrap();
        assert_eq!(encoded.bytes().len(), size);
    }
}

#[test]
fn subtract_immediate_lea_is_negative_canonical_and_flag_transparent() {
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    let rax = physical.model().view_named("rax").unwrap().id;
    let r12 = physical.model().view_named("r12").unwrap().id;
    let fact = AcceptedObligationFactIdentity::from_bytes([6; 32]);
    for (base, immediate, expected) in [
        (rax, 0, vec![0x48, 0x8d, 0x40, 0x00]),
        (rax, 128, vec![0x48, 0x8d, 0x40, 0x80]),
        (rax, 129, vec![0x48, 0x8d, 0x80, 0x7f, 0xff, 0xff, 0xff]),
        (
            r12,
            4095,
            vec![0x49, 0x8d, 0x84, 0x24, 0x01, 0xf0, 0xff, 0xff],
        ),
    ] {
        let kind = SelectedInstructionKind::ExactSubtractI64Immediate {
            immediate: IntegerValue::Unsigned(immediate),
            obligation: ObligationId::new(4).unwrap(),
            accepted_fact: fact,
        };
        let alternative = alternative(MachineAlternativeFamily::ExactSubtractI64Immediate, 0);
        let encoded =
            encode_x86_64_selected_form(&physical, kind, alternative, &[base, rax]).unwrap();
        assert_eq!(encoded.bytes(), expected);
        assert!(!encoded.footprint().writes_rflags);
        assert!(
            encoded
                .footprint()
                .encoded
                .implicit_unit_clobbers
                .is_empty()
        );
        let mut wrong_sign = expected;
        *wrong_sign.last_mut().unwrap() ^= 0x80;
        assert!(
            validate_x86_64_selected_form_encoding(
                &physical,
                kind,
                alternative,
                &[base, rax],
                &wrong_sign,
            )
            .is_err()
        );
    }
}

#[test]
fn materialize_i64_variant_one_is_the_optimizer_selected_xor_zero_idiom() {
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    let zero = SelectedInstructionKind::MaterializeI64 {
        value: IntegerValue::Unsigned(0),
    };
    let xor_zero = alternative(MachineAlternativeFamily::MaterializeI64, 1);
    for (name, bytes) in [
        ("rax", [0x48, 0x31, 0xc0]),
        ("rdi", [0x48, 0x31, 0xff]),
        ("r8", [0x4d, 0x31, 0xc0]),
        ("r15", [0x4d, 0x31, 0xff]),
    ] {
        let destination = [physical.model().view_named(name).unwrap().id];
        let encoded = encode_x86_64_selected_form(&physical, zero, xor_zero, &destination).unwrap();
        assert_eq!(encoded.bytes(), bytes);
        assert!(encoded.footprint().writes_rflags);
        assert_eq!(
            encoded.footprint().encoded.implicit_unit_clobbers,
            physical.model().view_named("rflags").unwrap().units
        );
        assert_eq!(encoded.footprint().register_writes, destination);
        assert!(encoded.footprint().register_reads.is_empty());
        validate_x86_64_selected_form_encoding(&physical, zero, xor_zero, &destination, &bytes)
            .unwrap();
        // The baseline ten-byte move is not the variant-one form and the
        // variant-one bytes are not the baseline.
        let baseline = alternative(MachineAlternativeFamily::MaterializeI64, 0);
        let moved = encode_x86_64_selected_form(&physical, zero, baseline, &destination).unwrap();
        assert_eq!(moved.bytes().len(), 10);
        assert!(!moved.footprint().writes_rflags);
        assert!(
            validate_x86_64_selected_form_encoding(&physical, zero, baseline, &destination, &bytes)
                .is_err()
        );
        assert!(
            validate_x86_64_selected_form_encoding(
                &physical,
                zero,
                xor_zero,
                &destination,
                moved.bytes()
            )
            .is_err()
        );
        crate::selected_form_encoding::assert_byte_fragility(&bytes, |candidate| {
            validate_x86_64_selected_form_encoding(
                &physical,
                zero,
                xor_zero,
                &destination,
                candidate,
            )
            .is_err()
        });
    }
    // Only an exact zero has the three-byte form.
    let destination = [physical.model().view_named("rax").unwrap().id];
    for value in [IntegerValue::Signed(-1), IntegerValue::Unsigned(1)] {
        assert_eq!(
            encode_x86_64_selected_form(
                &physical,
                SelectedInstructionKind::MaterializeI64 { value },
                xor_zero,
                &destination,
            ),
            Err(X86_64SelectedFormEncodingError::AlternativeMismatch)
        );
    }
    // The catalog declares the form once, tagged with its owning rule, so
    // the legality rule never chooses it and the rule finds it by tag.
    let constraints = crate::validate_x86_64_register_constraint_catalog(
        crate::x86_64_register_constraint_catalog(&physical),
        &physical,
    )
    .unwrap();
    let catalog =
        crate::x86_64_machine_effect_catalog(target::NativeTarget::linux_x64(), &constraints)
            .unwrap();
    let declaration = catalog
        .declarations
        .iter()
        .find(|declaration| {
            declaration.semantic == selected_instructions::MachineSemanticKind::MaterializeI64
        })
        .unwrap();
    assert_eq!(declaration.alternatives.len(), 2);
    assert_eq!(
        declaration.alternatives[1].applicability,
        selected_instructions::MachineAlternativeApplicability::OptimizationSelected {
            optimization: optimization_core::Optimization::X86SelectXorZeroI64MaterializationV1,
        }
    );
    assert_eq!(
        declaration.alternatives[1].size,
        selected_instructions::MachineSizeKnowledge::ExactBytes(3)
    );
    assert_eq!(
        declaration.alternatives[1].encoded.implicit_unit_clobbers,
        physical.model().view_named("rflags").unwrap().units
    );
}
