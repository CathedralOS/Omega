use super::*;
use crate::wire::{append_wire_byte_predicate_checks, wire_byte_predicate_checks_width};

#[cfg(test)]
mod integer_to_float_conversion_tests {
    use super::*;

    #[test]
    fn conversion_sequences_stay_in_width_lockstep() {
        for source_byte_size in [1usize, 2, 4, 8] {
            for source_signed in [false, true] {
                for target_byte_size in [4usize, 8] {
                    let mut bytes = Vec::new();
                    append_int_to_float_conversion(
                        &mut bytes,
                        source_byte_size,
                        target_byte_size,
                        source_signed,
                    );
                    assert_eq!(
                        bytes.len(),
                        int_to_float_conversion_width(source_byte_size, source_signed),
                        "int{} signed={source_signed} -> f{}",
                        source_byte_size * 8,
                        target_byte_size * 8,
                    );
                }
            }
        }
    }

    #[test]
    fn unsigned_u64_conversion_has_the_sticky_half_then_double_path() {
        let mut bytes = Vec::new();
        append_int_to_float_conversion(&mut bytes, 8, 8, false);
        assert_eq!(&bytes[..5], &[0x4d, 0x85, 0xd2, 0x79, 0x18]);
        assert!(
            bytes
                .windows(4)
                .any(|window| window == [0xf2, 0x0f, 0x58, 0xc0]),
            "upper-half u64 values must double the sticky half conversion",
        );
    }
}

#[cfg(test)]
mod float_to_integer_policy_tests {
    use super::*;

    #[test]
    fn policy_sequences_stay_in_width_lockstep() {
        for source_byte_size in [4usize, 8] {
            for target_byte_size in [1usize, 2, 4, 8] {
                for target_signed in [false, true] {
                    let mut trapping = Vec::new();
                    append_float_to_int_trap(
                        &mut trapping,
                        source_byte_size,
                        target_byte_size,
                        target_signed,
                    );
                    assert_eq!(
                        trapping.len(),
                        float_to_int_trap_width(source_byte_size, target_byte_size, target_signed,),
                        "Trapping f{source_byte_size}->int{target_byte_size} signed={target_signed} width"
                    );

                    let mut saturating = Vec::new();
                    append_float_to_int_saturating(
                        &mut saturating,
                        source_byte_size,
                        target_byte_size,
                        target_signed,
                    );
                    assert_eq!(
                        saturating.len(),
                        float_to_int_saturating_width(
                            source_byte_size,
                            target_byte_size,
                            target_signed,
                        ),
                        "Saturating f{source_byte_size}->int{target_byte_size} signed={target_signed} width"
                    );
                }
            }
        }
    }

    #[test]
    fn exact_conversion_keeps_the_zero_guard_cost() {
        assert_eq!(
            runtime_convert_operation_width(8, 4, true, false, false, true, false, false),
            10,
        );
        assert_eq!(
            runtime_convert_operation_width(8, 4, true, false, false, true, true, false),
            5 + float_to_int_trap_width(8, 4, true),
        );
        assert_eq!(
            runtime_convert_operation_width(8, 4, true, false, false, true, false, true),
            5 + float_to_int_saturating_width(8, 4, true),
        );
    }

    #[test]
    fn bounds_describe_truncation_not_only_integer_membership() {
        let (upper, lower, lower_inclusive) = float_to_int_bounds(8, 4, true);
        assert_eq!(f64::from_bits(upper), 2147483648.0);
        assert_eq!(f64::from_bits(lower), -2147483649.0);
        assert!(!lower_inclusive, "-2147483648.5 truncates into i32");

        let (upper, lower, lower_inclusive) = float_to_int_bounds(4, 4, true);
        assert_eq!(f32::from_bits(upper as u32), 2147483648.0);
        assert_eq!(f32::from_bits(lower as u32), -2147483648.0);
        assert!(lower_inclusive, "f32 cannot represent i32::MIN - 1");

        let (upper, lower, lower_inclusive) = float_to_int_bounds(8, 4, false);
        assert_eq!(f64::from_bits(upper), 4294967296.0);
        assert_eq!(f64::from_bits(lower), -1.0);
        assert!(!lower_inclusive, "-0.5 truncates into u32");
    }
}

#[cfg(test)]
mod float_arithmetic_policy_tests {
    use super::*;

    #[test]
    fn policy_sequences_stay_in_width_lockstep() {
        for byte_size in [4usize, 8] {
            for operator in [
                StateGuardOperator::Add,
                StateGuardOperator::AddTowardZero,
                StateGuardOperator::AddTowardPositive,
                StateGuardOperator::AddTowardNegative,
                StateGuardOperator::Subtract,
                StateGuardOperator::SubtractTowardZero,
                StateGuardOperator::SubtractTowardPositive,
                StateGuardOperator::SubtractTowardNegative,
                StateGuardOperator::Multiply,
                StateGuardOperator::MultiplyTowardZero,
                StateGuardOperator::MultiplyTowardPositive,
                StateGuardOperator::MultiplyTowardNegative,
                StateGuardOperator::Divide,
                StateGuardOperator::DivideTowardZero,
                StateGuardOperator::DivideTowardPositive,
                StateGuardOperator::DivideTowardNegative,
                StateGuardOperator::SqrtTowardZero,
                StateGuardOperator::SqrtTowardPositive,
                StateGuardOperator::SqrtTowardNegative,
                StateGuardOperator::MultiplyThenAdd,
            ] {
                for domain in [
                    ArithmeticDomain::Exact,
                    ArithmeticDomain::Saturating,
                    ArithmeticDomain::Trapping,
                ] {
                    let mut bytes = Vec::new();
                    append_runtime_float_binary_operation(&mut bytes, operator, byte_size, domain)
                        .expect("encode float operation");
                    assert_eq!(
                        bytes.len(),
                        runtime_float_binary_operation_width_with_domain(
                            operator, byte_size, domain,
                        ),
                        "f{} {operator:?} {domain:?} width",
                        byte_size * 8,
                    );
                }
            }
        }
    }

    #[test]
    fn directed_operations_balance_mxcsr_and_widths() {
        for (operator, mxcsr, opcode) in [
            (StateGuardOperator::AddTowardNegative, 0x3f80_u32, 0x58),
            (StateGuardOperator::AddTowardPositive, 0x5f80_u32, 0x58),
            (StateGuardOperator::AddTowardZero, 0x7f80_u32, 0x58),
            (StateGuardOperator::SubtractTowardNegative, 0x3f80_u32, 0x5c),
            (StateGuardOperator::SubtractTowardPositive, 0x5f80_u32, 0x5c),
            (StateGuardOperator::SubtractTowardZero, 0x7f80_u32, 0x5c),
            (StateGuardOperator::MultiplyTowardNegative, 0x3f80_u32, 0x59),
            (StateGuardOperator::MultiplyTowardPositive, 0x5f80_u32, 0x59),
            (StateGuardOperator::MultiplyTowardZero, 0x7f80_u32, 0x59),
            (StateGuardOperator::DivideTowardNegative, 0x3f80_u32, 0x5e),
            (StateGuardOperator::DivideTowardPositive, 0x5f80_u32, 0x5e),
            (StateGuardOperator::DivideTowardZero, 0x7f80_u32, 0x5e),
            (StateGuardOperator::SqrtTowardNegative, 0x3f80_u32, 0x51),
            (StateGuardOperator::SqrtTowardPositive, 0x5f80_u32, 0x51),
            (StateGuardOperator::SqrtTowardZero, 0x7f80_u32, 0x51),
        ] {
            for byte_size in [4usize, 8] {
                let mut bytes = Vec::new();
                append_runtime_float_binary_operation(
                    &mut bytes,
                    operator,
                    byte_size,
                    ArithmeticDomain::Exact,
                )
                .expect("encode directed operation");
                assert_eq!(
                    bytes.len(),
                    runtime_float_binary_operation_width_with_domain(
                        operator,
                        byte_size,
                        ArithmeticDomain::Exact,
                    )
                );
                assert_eq!(
                    &bytes[10..18],
                    &[0x48, 0x83, 0xec, 0x10, 0x0f, 0xae, 0x1c, 0x24]
                );
                assert_eq!(&bytes[22..26], &mxcsr.to_le_bytes());
                assert_eq!(bytes[33], opcode);
                assert_eq!(
                    &bytes[35..43],
                    &[0x0f, 0xae, 0x14, 0x24, 0x48, 0x83, 0xc4, 0x10]
                );
            }
        }
    }

    #[test]
    fn classification_sequences_stay_in_width_lockstep() {
        for byte_size in [4usize, 8] {
            for operator in [
                StateGuardOperator::IsFinite,
                StateGuardOperator::IsInfinite,
                StateGuardOperator::IsNormal,
                StateGuardOperator::IsSubnormal,
                StateGuardOperator::FloatClassify,
            ] {
                let mut bytes = Vec::new();
                append_runtime_float_binary_operation(
                    &mut bytes,
                    operator,
                    byte_size,
                    ArithmeticDomain::Exact,
                )
                .expect("encode float classification");
                assert_eq!(
                    bytes.len(),
                    runtime_float_binary_operation_width_with_domain(
                        operator,
                        byte_size,
                        ArithmeticDomain::Exact,
                    ),
                    "f{} {operator:?} width",
                    byte_size * 8,
                );
            }
        }
    }

    #[test]
    fn policy_branches_target_emitted_labels() {
        for byte_size in [4usize, 8] {
            for operator in [
                StateGuardOperator::Add,
                StateGuardOperator::Subtract,
                StateGuardOperator::Multiply,
                StateGuardOperator::Divide,
                StateGuardOperator::MultiplyThenAdd,
            ] {
                for domain in [ArithmeticDomain::Saturating, ArithmeticDomain::Trapping] {
                    let include_middle = operator == StateGuardOperator::MultiplyThenAdd;
                    let bytes =
                        float_policy_guard_bytes(domain, operator, byte_size, include_middle)
                            .expect("encode policy guard");
                    let mut branches = 0;
                    for start in 0..bytes.len().saturating_sub(5) {
                        if bytes[start] == 0x0f
                            && matches!(bytes[start + 1], 0x82 | 0x83 | 0x84 | 0x85 | 0x87)
                        {
                            let displacement = i32::from_le_bytes(
                                bytes[start + 2..start + 6].try_into().expect("rel32 bytes"),
                            );
                            let target = (start + 6) as isize + displacement as isize;
                            assert!(
                                target >= 0 && target as usize <= bytes.len(),
                                "branch at {start} targets {target}, outside {} bytes",
                                bytes.len(),
                            );
                            branches += 1;
                        }
                    }
                    let minimum_branches = match (domain, include_middle) {
                        (ArithmeticDomain::Trapping, _) => 1,
                        (_, true) => 4,
                        _ => 3,
                    };
                    assert!(branches >= minimum_branches);
                    assert_eq!(
                        bytes.windows(2).any(|window| window == [0x0f, 0x0b]),
                        domain == ArithmeticDomain::Trapping,
                        "only Trapping emits ud2",
                    );
                }
            }
        }
    }

    #[test]
    fn multiply_then_add_emits_two_scalar_operations_without_contraction() {
        for (byte_size, prefix) in [(4usize, 0xf3), (8, 0xf2)] {
            let bytes = float_multiply_then_add_bytes(byte_size, ArithmeticDomain::Exact)
                .expect("encode exact multiply-then-add");
            assert!(
                bytes
                    .windows(4)
                    .any(|window| window == [prefix, 0x0f, 0x59, 0xc1]),
                "f{} must contain a scalar multiply",
                byte_size * 8,
            );
            assert!(
                bytes
                    .windows(4)
                    .any(|window| window == [prefix, 0x0f, 0x58, 0xc2]),
                "f{} must contain a separate scalar add",
                byte_size * 8,
            );
            assert_eq!(
                bytes.len(),
                runtime_float_binary_operation_width_with_domain(
                    StateGuardOperator::MultiplyThenAdd,
                    byte_size,
                    ArithmeticDomain::Exact,
                ),
            );
        }
    }

    #[test]
    fn float_comparisons_never_gain_policy_bytes() {
        for operator in [StateGuardOperator::Equal, StateGuardOperator::NotEqual] {
            assert!(
                float_policy_guard_bytes(ArithmeticDomain::Trapping, operator, 8, false)
                    .expect("gated guard")
                    .is_empty()
            );
        }
    }

    #[test]
    fn named_result_operations_receive_policy_bytes() {
        for operator in [
            StateGuardOperator::Min,
            StateGuardOperator::Max,
            StateGuardOperator::Sqrt,
        ] {
            assert!(
                !float_policy_guard_bytes(ArithmeticDomain::Trapping, operator, 8, false)
                    .expect("result policy guard")
                    .is_empty()
            );
        }
    }
}

#[cfg(test)]
mod wrapping_shift_clamp_tests {
    use super::*;

    #[test]
    fn clamp_compares_the_full_count_and_cmovs_zero() {
        for &byte_size in &[1usize, 2, 4, 8] {
            let mut bytes = Vec::new();
            append_wrapping_shift_zero_clamp(&mut bytes, byte_size);
            assert_eq!(
                bytes.len(),
                WRAPPING_SHIFT_ZERO_CLAMP_WIDTH,
                "width mismatch for the {byte_size}-byte clamp"
            );
            assert_eq!(&bytes[0..2], &[0x31, 0xc0], "xor eax, eax");
            // The FULL count in r11 (not the cl copy): a count with set high
            // bits (i64 2^32+1, or a negative signed count) must still clamp.
            assert_eq!(&bytes[2..5], &[0x49, 0x83, 0xfb], "cmp r11, imm8");
            assert_eq!(bytes[5] as usize, byte_size * 8, "width_bits immediate");
            assert_eq!(&bytes[6..10], &[0x4c, 0x0f, 0x43, 0xd0], "cmovae r10, rax");
        }
    }

    #[test]
    fn wrapping_count_mask_masks_subword_only() {
        // F8b: the Wrapping count mask is an explicit AND at sub-word widths
        // (`and r11d, 7/15`) and ABSENT at 4/8 -- the hardware `shl`/`sar`
        // mask mod 32/64 there, which IS the ch5 masked-count ruling.
        for &(byte_size, expect) in &[(1usize, Some(7u8)), (2, Some(15)), (4, None), (8, None)] {
            let mut bytes = Vec::new();
            append_wrapping_shift_count_mask(&mut bytes, byte_size);
            assert_eq!(
                bytes.len(),
                wrapping_shift_count_mask_width(byte_size),
                "emission and width accounting must agree at {byte_size} bytes"
            );
            match expect {
                Some(mask) => assert_eq!(bytes, vec![0x41, 0x83, 0xe3, mask], "and r11d, mask"),
                None => assert!(bytes.is_empty(), "no mask at width {byte_size}"),
            }
        }
    }

    #[test]
    fn wrapping_shl_sequence_shifts_then_clamps_without_touching_the_count() {
        // The write-path pair: width-correct shl (hardware masks the count
        // mod 32) followed by the modular clamp reading the intact r11.
        let mut bytes = Vec::new();
        append_runtime_binary_operation(&mut bytes, StateGuardOperator::ShiftLeft, 4).expect("shl");
        append_wrapping_shift_zero_clamp(&mut bytes, 4);
        assert_eq!(
            bytes,
            vec![
                0x44, 0x89, 0xd9, // mov ecx, r11d (count copy; r11 stays intact)
                0x41, 0xd3, 0xe2, // shl r10d, cl
                0x31, 0xc0, // xor eax, eax
                0x49, 0x83, 0xfb, 32, // cmp r11, 32
                0x4c, 0x0f, 0x43, 0xd0, // cmovae r10, rax
            ]
        );
        assert_eq!(
            bytes.len(),
            runtime_binary_operation_width(StateGuardOperator::ShiftLeft, 4)
                + WRAPPING_SHIFT_ZERO_CLAMP_WIDTH,
            "emission and width accounting must agree"
        );
    }

    #[test]
    fn arithmetic_shr_saturates_the_count_before_the_sar() {
        // The pre-fix: at/above-width counts become width-1, so the sar
        // itself produces the sign-fill. cmovae writes r11 (the count),
        // NOT r10 (the value).
        let mut bytes = Vec::new();
        append_wrapping_shift_right_count_saturate(&mut bytes, 4);
        append_runtime_binary_operation(&mut bytes, StateGuardOperator::ShiftRight, 4)
            .expect("sar");
        assert_eq!(
            bytes,
            vec![
                0xb8, 31, 0, 0, 0, // mov eax, 31 (width-1)
                0x49, 0x83, 0xfb, 32, // cmp r11, 32
                0x4c, 0x0f, 0x43, 0xd8, // cmovae r11, rax (count, not value)
                0x44, 0x89, 0xd9, // mov ecx, r11d (saturated count copy)
                0x41, 0xd3, 0xfa, // sar r10d, cl
            ]
        );
        assert_eq!(
            bytes.len(),
            WRAPPING_SHIFT_RIGHT_COUNT_SATURATE_WIDTH
                + runtime_binary_operation_width(StateGuardOperator::ShiftRight, 4),
            "emission and width accounting must agree"
        );
    }

    #[test]
    fn saturating_trapping_shift_left_width_stays_in_lockstep() {
        // Every (domain x signedness x width) arm's emitted length must
        // match the width twin, or relocation offsets drift.
        for domain in [ArithmeticDomain::Saturating, ArithmeticDomain::Trapping] {
            for target_signed in [false, true] {
                for byte_size in [1usize, 2, 4, 8] {
                    let mut bytes = Vec::new();
                    append_saturating_trapping_shift_left(
                        &mut bytes,
                        domain,
                        byte_size,
                        target_signed,
                    )
                    .expect("emit");
                    assert_eq!(
                        bytes.len(),
                        saturating_trapping_shift_left_width(domain, byte_size, target_signed),
                        "width mismatch: {domain:?} signed={target_signed} {byte_size}b"
                    );
                }
            }
        }
    }

    #[test]
    fn saturating_shl_narrow_caps_the_count_then_takes_the_unsigned_upper_clamp() {
        // u8 Saturating: [cap count at 8] + 64-bit shl + cmova against 255.
        let mut bytes = Vec::new();
        append_saturating_trapping_shift_left(&mut bytes, ArithmeticDomain::Saturating, 1, false)
            .expect("emit");
        assert_eq!(
            bytes,
            vec![
                0xb8, 8, 0, 0, 0, // mov eax, 8 (the width)
                0x49, 0x83, 0xfb, 8, // cmp r11, 8
                0x4c, 0x0f, 0x43, 0xd8, // cmovae r11, rax (cap the COUNT)
                0x4c, 0x89, 0xd9, // mov rcx, r11
                0x49, 0xd3, 0xe2, // shl r10, cl (64-bit, exact)
                0x49, 0xbb, 255, 0, 0, 0, 0, 0, 0, 0, // mov r11, 255
                0x4d, 0x39, 0xda, // cmp r10, r11
                0x4d, 0x0f, 0x47, 0xd3, // cmova r10, r11 (UNSIGNED upper)
            ]
        );
    }

    #[test]
    fn saturating_trapping_add_sub_width_stays_in_lockstep() {
        // Every (domain x op x signedness x width x per-side-immediate) arm's
        // emitted length must match the width twin, or relocation offsets
        // drift.
        for domain in [ArithmeticDomain::Saturating, ArithmeticDomain::Trapping] {
            for operator in [StateGuardOperator::Add, StateGuardOperator::Subtract] {
                for target_signed in [false, true] {
                    for byte_size in [1usize, 2, 4, 8] {
                        for left_imm in [false, true] {
                            for right_imm in [false, true] {
                                let mut bytes = Vec::new();
                                append_saturating_trapping_add_sub(
                                    &mut bytes,
                                    domain,
                                    operator,
                                    byte_size,
                                    target_signed,
                                    left_imm,
                                    right_imm,
                                )
                                .expect("emit");
                                assert_eq!(
                                    bytes.len(),
                                    saturating_trapping_add_sub_width(
                                        domain,
                                        operator,
                                        byte_size,
                                        target_signed,
                                        left_imm,
                                        right_imm,
                                    ),
                                    "width mismatch: {domain:?} {operator:?} \
                                     signed={target_signed} {byte_size}b \
                                     imm=({left_imm},{right_imm})"
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn min_idiom_subtract_skips_the_immediate_and_wide_computes() {
        // The MIN idiom `(0 as i32 in Saturating) - 2147483648`: left is a
        // convert (extends), right is a WIDE immediate (must NOT re-extend);
        // one exact 64-bit sub; both signed bounds.
        let mut bytes = Vec::new();
        append_saturating_trapping_add_sub(
            &mut bytes,
            ArithmeticDomain::Saturating,
            StateGuardOperator::Subtract,
            4,
            true,
            false, // left: convert-of-literal, not an immediate operand
            true,  // right: the wide literal
        )
        .expect("emit");
        assert_eq!(
            &bytes[0..3],
            &[0x4d, 0x63, 0xd2],
            "movsxd r10 (left extends)"
        );
        // NO movsxd r11 (4d 63 db) anywhere: the immediate keeps its wide value.
        assert!(
            !bytes.windows(3).any(|w| w == [0x4d, 0x63, 0xdb]),
            "the immediate operand must not re-extend"
        );
        assert_eq!(
            &bytes[3..6],
            &[0x4d, 0x29, 0xda],
            "wide 64-bit sub r10, r11"
        );
    }

    #[test]
    fn unsigned_saturating_subtract_clamps_downward_with_a_signed_compare() {
        // 10u8 - 100u8 wide-computes to -90, whose UNSIGNED reading is huge:
        // the subtract arm clamps to 0 through cmovl (signed), never cmova.
        let mut bytes = Vec::new();
        append_saturating_trapping_add_sub(
            &mut bytes,
            ArithmeticDomain::Saturating,
            StateGuardOperator::Subtract,
            1,
            false,
            false,
            false,
        )
        .expect("emit");
        assert!(
            bytes.windows(4).any(|w| w == [0x4d, 0x0f, 0x4c, 0xd3]),
            "expected cmovl (signed lower clamp to 0)"
        );
        assert!(
            !bytes.windows(4).any(|w| w == [0x4d, 0x0f, 0x47, 0xd3]),
            "an unsigned upper cmova would clamp underflow to MAX"
        );
    }

    #[test]
    fn wire_byte_predicate_checks_emit_deterministically() {
        // The width fn measures the pure emitter; determinism and the
        // block-prefix bytes are the executable-free sanity available on
        // this host (runtime behavior rides the linux_x64 ELF pin + the
        // differential once an x86 host runs the suite).
        use psi_language_semantics::byte_predicates::ByteSequencePredicate;
        for predicate in ByteSequencePredicate::ALL {
            let mask = predicate.mask_bit();
            let mut once = Vec::new();
            append_wire_byte_predicate_checks(&mut once, mask);
            let mut twice = Vec::new();
            append_wire_byte_predicate_checks(&mut twice, mask);
            assert_eq!(once, twice, "{predicate:?} must emit deterministically");
            assert_eq!(once.len(), wire_byte_predicate_checks_width(mask));
            assert!(!once.is_empty(), "{predicate:?} must emit a check");
            // Every block ends able to clear the ok flag: xor r9d, r9d.
            assert!(
                once.windows(3).any(|w| w == [0x45, 0x31, 0xc9]),
                "{predicate:?} must clear r9d on violation"
            );
        }
        // The utf8 walk begins with the pointer/end setup shared by the
        // loop blocks: mov rcx, r15 / mov r11, r15 / add r11, rax.
        let mut utf8 = Vec::new();
        append_wire_byte_predicate_checks(&mut utf8, ByteSequencePredicate::ValidUtf8.mask_bit());
        assert_eq!(
            &utf8[0..9],
            &[0x4c, 0x89, 0xf9, 0x4d, 0x89, 0xfb, 0x49, 0x01, 0xc3]
        );
    }

    #[test]
    fn node_width_extension_width_stays_in_lockstep() {
        for &byte_width in &[1usize, 2, 4, 8] {
            for &operands_signed in &[false, true] {
                let mut bytes = Vec::new();
                append_wrapping_node_width_extension(&mut bytes, byte_width, operands_signed);
                assert_eq!(
                    bytes.len(),
                    wrapping_node_width_extension_width(byte_width),
                    "extension width mismatch at {byte_width} bytes (signed: {operands_signed})"
                );
            }
        }
    }
}

// ============================================================================
