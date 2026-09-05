use super::*;

const MIXED_NOMINAL_SHARED_INTEGER_COMPARISON_CONVERGENCE_SOURCE: &str = r#"
    data Helper {}
    machine Helper::touch() {}

    data Token {}
    machine Token::drop(&mut self) { Helper::touch(); }
    data Root {}
    machine Root::measure(
        token: Token,
        input: u64 in Wrapping,
        small: u8,
        divisor: u8,
        count: u8,
        signed: i64,
        signed_arithmetic: i8,
        signed_divisor: i8,
        negative_divisor: i8,
        bounded_negative_divisor: i8,
        add_left: u8,
        add_right: u8,
        positive_addend: i8,
        negative_addend: i8,
        positive_subtrahend: i8,
        negative_subtrahend: i8,
        signed_count: i8,
        enabled: bool,
        wide: u16
    ) -> bool
    requires input <= 255u64, input <= 250u64, input <= 253u64, input <= 252u64,
        input <= 251u64, input <= 127u64, input <= 125u64, input <= 124u64,
        input <= 42u64, input <= 31u64,
        5u64 <= input, input <= 260u64,
        small <= 254u8, small <= 253u8, small <= 252u8,
        small <= 127u8, small <= 125u8, small <= 124u8, small <= 61u8,
        small <= 63u8, small <= 42u8, small <= 31u8,
        small <= 21u8, small <= 15u8,
        small <= 7u8, 1u8 <= small, 2u8 <= small, 3u8 <= small,
        1u8 <= divisor, divisor <= small,
        small <= 255u8 / divisor, count <= 2u8,
        -128i64 <= signed, signed <= 127i64,
        -125i64 <= signed, signed <= 130i64,
        -61i64 <= signed, signed <= 66i64,
        -64i64 <= signed, signed <= 63i64, -21i64 <= signed, signed <= 21i64,
        -16i64 <= signed, signed <= 15i64,
        -127i8 <= signed_arithmetic, signed_arithmetic <= 126i8,
        -126i8 <= signed_arithmetic, -125i8 <= signed_arithmetic,
        signed_arithmetic <= 124i8,
        -42i8 <= signed_arithmetic, signed_arithmetic <= 42i8,
        -61i8 <= signed_arithmetic, signed_arithmetic <= 66i8,
        -32i8 <= signed_arithmetic, signed_arithmetic <= 31i8,
        -3i8 <= signed_arithmetic, -1i8 <= signed_arithmetic, 0i8 <= signed_arithmetic,
        3i8 <= signed_arithmetic,
        1i8 <= signed_arithmetic, 0i8 <= signed_divisor,
        1i8 <= signed_divisor, signed_divisor <= 7i8,
        -128i8 / signed_divisor <= signed_arithmetic,
        signed_arithmetic <= 127i8 / signed_divisor,
        negative_divisor <= -2i8, bounded_negative_divisor <= -1i8,
        127i8 / negative_divisor <= signed_arithmetic,
        signed_arithmetic <= -128i8 / negative_divisor,
        add_left <= 255u8 - add_right,
        0i8 <= positive_addend, signed_arithmetic <= 127i8 - positive_addend,
        negative_addend <= 0i8, -128i8 - negative_addend <= signed_arithmetic,
        0i8 <= positive_subtrahend, -128i8 + positive_subtrahend <= signed_arithmetic,
        negative_subtrahend <= 0i8, signed_arithmetic <= 127i8 + negative_subtrahend,
        0i8 <= signed_count, signed_count <= 2i8
    {
        let staged: bool = ((((input + 1u64) < 4u64) || ((~input) < 1u64) || (input <= 9u64))
            && (((input + 1u64) + 1u64) < 5u64)
            && ((small as u16) < 5u16))
            && ((input as u8) < 5u8)
            && (((input as u8) as u16) < 256u16)
            && (((small as u16) as u8) < 6u8)
            && (((((small as u16) as u32) as u64) as u8) < 7u8)
            && ((small + 1u8) < 6u8)
            && ((((small + 1u8) + 1u8) + 1u8) < 8u8)
            && ((~(small + 3u8)) < 255u8)
            && (((small - 3u8) as u16) < 255u16)
            && ((((small - 1u8) - 1u8) - 1u8) < 5u8)
            && ((15u8 & (small * 2u8)) < 16u8)
            && ((~((small + 3u8) as u16)) < 65535u16)
            && (((small + 1u8) & (small * 2u8)) < 255u8)
            && ((127u8 - small) < 125u8)
            && ((small - divisor) < 4u8)
            && ((small * 2u8) < 10u8)
            && ((((small * 2u8) * 3u8) * 1u8) < 255u8)
            && (((((small + 3u8) * 2u8) - 1u8) < 255u8))
            && (((((small + 3u8) * 0u8) + 255u8) < 255u8))
            && (((((signed_arithmetic + -3i8) * 2i8) - -1i8) < 127i8))
            && (((((((small + 3u8) * 2u8) - 1u8) as i8) < 127i8)
                && (((((small + 3u8) * 0u8) + 127u8) as i8) < 127i8))
                && (((((signed_arithmetic - 3i8) * 2i8) + 1i8) as u8) < 255u8))
            && (((((small * 2u8) * 3u8) as i8) < 127i8))
            && (((((small * 2u8) * 0u8) as i8) < 127i8))
            && ((small * divisor) < 50u8)
            && (((small / 2u8) < 3u8)
                && ((small % 2u8) <= 1u8)
                && (((((small / 2u8) % 3u8) / 2u8) < 2u8)
                && (((((input as u8) / 2u8) % 3u8) / 2u8) < 2u8)
                && ((((signed as i8) / 2i8) % -3i8) < 3i8)
                && ((((signed_arithmetic as u8) / 2u8) % 3u8) < 3u8)
                && ((((wide / 256u16) as u8) < 255u8)
                    && ((((wide / 2u16) % 3u16) as u8) < 3u8)
                    && (((signed % -3i64) as i8) < 3i8)
                    && ((signed / -1i64) <= 128i64)
                    && ((signed % -1i64) <= 0i64)
                    && (((wide % 3u16) as i8) < 3i8)
                    && ((((small / divisor) % 2u8) < 2u8)
                        && ((((input as u8) / divisor) % 2u8) < 2u8)
                        && (((signed_arithmetic / signed_divisor) % -3i8) < 3i8)
                        && (((signed_arithmetic / negative_divisor) % 3i8) < 3i8)
                        && ((((signed as i8) / signed_divisor) % -3i8) < 3i8)
                        && ((((signed as i8) / negative_divisor) % 3i8) < 3i8)))))
            && ((small / divisor) < 6u8)
            && ((small % divisor) <= small)
            && ((small >> small) < 1u8)
            && ((signed_arithmetic >> signed_divisor) < 4i8)
            && ((((small >> 1i8) >> 2u16) >> 0i32) < 2u8)
            && (((((small >> 1i8) >> 2u16) >> 0i32) as i8) < 127i8)
            && (((small >> 0i8) as i8) < 127i8)
            && ((((small << 1i8) << 2u16) << 0i32) < 255u8)
            && (((((small << 1i8) << 2u16) << 0i32) as i8) < 127i8)
            && (((small << 0i8) as i8) < 127i8)
            && ((small << 1u8) < 11u8)
            && ((small << count) < 29u8)
            && ((small << signed_count) < 255u8)
            && ((signed_arithmetic << 2u8) < 127i8)
            && ((signed_arithmetic << count) < 127i8)
            && ((signed_arithmetic << signed_count) < 127i8)
            && ((signed as i8) < 4i8)
            && ((small as i8) < 4i8)
            && ((signed_arithmetic as u8) < 4u8)
            && ((signed_arithmetic + 1i8) < 4i8)
            && ((signed_arithmetic + -1i8) < 4i8)
            && ((signed_arithmetic - 1i8) < 4i8)
            && ((signed_arithmetic - -1i8) < 4i8)
            && ((((small + 3u8) - 2u8) + 1u8) < 255u8)
            && ((((signed_arithmetic - -3i8) + -5i8) - -1i8) < 127i8)
            && (((((small + 3u8) - 2u8) + 1u8) as i8) < 127i8)
            && (((((signed_arithmetic - -3i8) + -5i8) - -1i8) as u8) < 127u8)
            && (((input as u8) + 5u8) < 255u8)
            && (((input as u8) - 5u8) < 255u8)
            && (((((input as u8) + 5u8) - 3u8) + 2u8) < 255u8)
            && ((((input as u8) + 5u8) - 5u8) < 255u8)
            && (((signed_arithmetic as u8) + 1u8) < 255u8)
            && ((((signed_arithmetic as u8) + 3u8) - 2u8) < 255u8)
            && ((((input as u8) * 2u8) * 3u8) < 255u8)
            && ((((input as u8) * 2u8) * 0u8) < 255u8)
            && ((((((input as u8) + 3u8) * 2u8) - 1u8) < 255u8)
                && (((((input as u8) + 3u8) * 0u8) + 255u8) < 255u8)
                && (((((signed as i8) - 3i8) * 2i8) + 1i8) < 127i8))
            && ((((signed as i8) * 2i8) * 3i8) < 127i8)
            && ((((signed_arithmetic as u8) * 2u8) * 3u8) < 255u8)
            && ((((small as i8) * 2i8) * 3i8) < 127i8)
            && (((((input as u8) << 1i8) << 2u16) << 0i32) < 255u8)
            && ((((signed as i8) << 1u16) << 2i32) < 127i8)
            && ((((signed_arithmetic as u8) << 1i8) << 2u16) < 255u8)
            && (((((small as i8) << 1u16) << 2i32) < 127i8)
                && (((((input as u8) >> 1i8) >> 2u16) >> 0i32) < 255u8)
                && ((((signed as i8) >> 1u16) >> 2i32) < 127i8)
                && ((((signed_arithmetic as u8) >> 1i8) >> 2u16) < 255u8))
            && ((signed_arithmetic * 3i8) < 4i8)
            && ((signed_arithmetic * -3i8) < 4i8)
            && ((signed_arithmetic * signed_divisor) <= 127i8)
            && ((signed_arithmetic * negative_divisor) <= 127i8)
            && ((signed_arithmetic / 2i8) < 4i8)
            && ((signed_arithmetic % -2i8) <= 1i8)
            && ((signed_arithmetic / -1i8) <= 127i8)
            && ((signed_arithmetic % -1i8) <= 0i8)
            && ((signed_arithmetic / signed_divisor) < 4i8)
            && ((signed_arithmetic % signed_divisor) <= signed_arithmetic)
            && ((signed_arithmetic / negative_divisor) < 4i8)
            && ((signed_arithmetic % negative_divisor) <= signed_arithmetic)
            && ((signed_arithmetic / bounded_negative_divisor) < 4i8)
            && ((signed_arithmetic % bounded_negative_divisor) <= signed_arithmetic)
            && ((add_left + add_right) <= 255u8)
            && ((signed_arithmetic + positive_addend) <= 127i8)
            && ((signed_arithmetic + negative_addend) < 4i8)
            && ((signed_arithmetic - positive_subtrahend) < 4i8)
            && ((signed_arithmetic - negative_subtrahend) <= 127i8)
            && (input == 3u64)
            && enabled;
        staged
    }
"#;

#[test]
#[rustfmt::skip]
fn mixed_nominal_integer_comparison_converges_before_one_shared_cleanup_return() {
    let tokens = Lexer::new(MIXED_NOMINAL_SHARED_INTEGER_COMPARISON_CONVERGENCE_SOURCE)
        .tokenize()
        .expect("tokenize shared integer-comparison convergence");
    let syntax = parse_syntax_trees(&tokens).expect("parse shared integer-comparison convergence");
    let resolved = lower_syntax_trees(&syntax).expect("resolve shared integer convergence");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type shared integer convergence");
    let checked = lower_typed_trees(typed).expect("check shared integer convergence");
    let lowered = checked_trees_to_terminal_psi::lower_machine(&checked, "Root::measure")
        .expect("shared integer-comparison convergence lowers");
    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("shared integer convergence entry");
    let unsigned_term = |bits: u16, value: u128| {
        ScalarTerm::integer(
            IntegerType::new(IntegerSign::Unsigned, bits).unwrap(),
            IntegerValue::Unsigned(value),
        )
        .unwrap_or_else(|error| panic!("test integer term: {error:?}"))
    };
    let input_term = ScalarTerm::value(entry.parameters[0].id, entry.parameters[0].scalar_type);
    let small_term = ScalarTerm::value(entry.parameters[1].id, entry.parameters[1].scalar_type);
    let divisor_term = ScalarTerm::value(entry.parameters[2].id, entry.parameters[2].scalar_type);
    let count_term = ScalarTerm::value(entry.parameters[3].id, entry.parameters[3].scalar_type);
    let signed_term = ScalarTerm::value(entry.parameters[4].id, entry.parameters[4].scalar_type);
    let signed_type = IntegerType::new(IntegerSign::Signed, 64).unwrap();
    let signed_arithmetic_term =
        ScalarTerm::value(entry.parameters[5].id, entry.parameters[5].scalar_type);
    let signed_arithmetic_type = IntegerType::new(IntegerSign::Signed, 8).unwrap();
    let signed_divisor_term =
        ScalarTerm::value(entry.parameters[6].id, entry.parameters[6].scalar_type);
    let negative_divisor_term =
        ScalarTerm::value(entry.parameters[7].id, entry.parameters[7].scalar_type);
    let bounded_negative_divisor_term =
        ScalarTerm::value(entry.parameters[8].id, entry.parameters[8].scalar_type);
    let add_left_term = ScalarTerm::value(entry.parameters[9].id, entry.parameters[9].scalar_type);
    let add_right_term =
        ScalarTerm::value(entry.parameters[10].id, entry.parameters[10].scalar_type);
    let positive_addend_term =
        ScalarTerm::value(entry.parameters[11].id, entry.parameters[11].scalar_type);
    let negative_addend_term =
        ScalarTerm::value(entry.parameters[12].id, entry.parameters[12].scalar_type);
    let positive_subtrahend_term =
        ScalarTerm::value(entry.parameters[13].id, entry.parameters[13].scalar_type);
    let negative_subtrahend_term =
        ScalarTerm::value(entry.parameters[14].id, entry.parameters[14].scalar_type);
    let signed_count_term =
        ScalarTerm::value(entry.parameters[15].id, entry.parameters[15].scalar_type);
    let input_upper_requirement =
        Proposition::LessOrEqual(input_term.clone(), unsigned_term(64, 255));
    let shift_upper_requirement = Proposition::LessOrEqual(small_term.clone(), unsigned_term(8, 7));
    let exact_upper_requirement =
        Proposition::LessOrEqual(small_term.clone(), unsigned_term(8, 127));
    let left_shift_value_requirement =
        Proposition::LessOrEqual(small_term.clone(), unsigned_term(8, 63));
    let add_upper_requirement = Proposition::LessOrEqual(small_term.clone(), unsigned_term(8, 254));
    let bitwise_not_exact_add_requirement =
        Proposition::LessOrEqual(small_term.clone(), unsigned_term(8, 252));
    let widen_exact_subtract_requirement =
        Proposition::LessOrEqual(unsigned_term(8, 3), small_term.clone());
    let divisor_lower_requirement =
        Proposition::LessOrEqual(unsigned_term(8, 1), divisor_term.clone());
    let runtime_subtract_requirement =
        Proposition::LessOrEqual(divisor_term.clone(), small_term.clone());
    let left_shift_count_requirement = Proposition::LessOrEqual(count_term, unsigned_term(8, 2));
    let signed_lower_requirement = Proposition::LessOrEqual(
        ScalarTerm::integer(signed_type, IntegerValue::Signed(-128)).unwrap(),
        signed_term.clone(),
    );
    let signed_upper_requirement = Proposition::LessOrEqual(
        signed_term,
        ScalarTerm::integer(signed_type, IntegerValue::Signed(127)).unwrap(),
    );
    let signed_arithmetic_lower_requirement = Proposition::LessOrEqual(
        ScalarTerm::integer(signed_arithmetic_type, IntegerValue::Signed(-127)).unwrap(),
        signed_arithmetic_term.clone(),
    );
    let signed_arithmetic_upper_requirement = Proposition::LessOrEqual(
        signed_arithmetic_term.clone(),
        ScalarTerm::integer(signed_arithmetic_type, IntegerValue::Signed(126)).unwrap(),
    );
    let signed_multiply_lower_requirement = Proposition::LessOrEqual(
        ScalarTerm::integer(signed_arithmetic_type, IntegerValue::Signed(-42)).unwrap(),
        ScalarTerm::value(entry.parameters[5].id, entry.parameters[5].scalar_type),
    );
    let signed_multiply_upper_requirement = Proposition::LessOrEqual(
        ScalarTerm::value(entry.parameters[5].id, entry.parameters[5].scalar_type),
        ScalarTerm::integer(signed_arithmetic_type, IntegerValue::Signed(42)).unwrap(),
    );
    let signed_shift_value_lower_requirement = Proposition::LessOrEqual(
        ScalarTerm::integer(signed_arithmetic_type, IntegerValue::Signed(-32)).unwrap(),
        ScalarTerm::value(entry.parameters[5].id, entry.parameters[5].scalar_type),
    );
    let signed_shift_value_upper_requirement = Proposition::LessOrEqual(
        ScalarTerm::value(entry.parameters[5].id, entry.parameters[5].scalar_type),
        ScalarTerm::integer(signed_arithmetic_type, IntegerValue::Signed(31)).unwrap(),
    );
    let signed_nonnegative_requirement = Proposition::LessOrEqual(
        ScalarTerm::integer(signed_arithmetic_type, IntegerValue::Signed(0)).unwrap(),
        ScalarTerm::value(entry.parameters[5].id, entry.parameters[5].scalar_type),
    );
    let signed_shift_count_lower_requirement = Proposition::LessOrEqual(
        ScalarTerm::integer(signed_arithmetic_type, IntegerValue::Signed(0)).unwrap(),
        signed_divisor_term.clone(),
    );
    let signed_shift_count_upper_requirement = Proposition::LessOrEqual(
        signed_divisor_term.clone(),
        ScalarTerm::integer(signed_arithmetic_type, IntegerValue::Signed(7)).unwrap(),
    );
    let signed_divisor_lower_requirement = Proposition::LessOrEqual(
        ScalarTerm::integer(signed_arithmetic_type, IntegerValue::Signed(1)).unwrap(),
        signed_divisor_term.clone(),
    );
    let runtime_signed_positive_multiply_lower_requirement = Proposition::LessOrEqual(
        ScalarTerm::exact_integer_divide(
            signed_arithmetic_type,
            ScalarTerm::integer(signed_arithmetic_type, IntegerValue::Signed(-128)).unwrap(),
            signed_divisor_term.clone(),
        )
        .unwrap(),
        signed_arithmetic_term.clone(),
    );
    let runtime_signed_positive_multiply_upper_requirement = Proposition::LessOrEqual(
        signed_arithmetic_term.clone(),
        ScalarTerm::exact_integer_divide(
            signed_arithmetic_type,
            ScalarTerm::integer(signed_arithmetic_type, IntegerValue::Signed(127)).unwrap(),
            signed_divisor_term,
        )
        .unwrap(),
    );
    let negative_divisor_upper_requirement = Proposition::LessOrEqual(
        negative_divisor_term.clone(),
        ScalarTerm::integer(signed_arithmetic_type, IntegerValue::Signed(-2)).unwrap(),
    );
    let runtime_signed_negative_multiply_lower_requirement = Proposition::LessOrEqual(
        ScalarTerm::exact_integer_divide(
            signed_arithmetic_type,
            ScalarTerm::integer(signed_arithmetic_type, IntegerValue::Signed(127)).unwrap(),
            negative_divisor_term.clone(),
        )
        .unwrap(),
        signed_arithmetic_term.clone(),
    );
    let runtime_signed_negative_multiply_upper_requirement = Proposition::LessOrEqual(
        signed_arithmetic_term.clone(),
        ScalarTerm::exact_integer_divide(
            signed_arithmetic_type,
            ScalarTerm::integer(signed_arithmetic_type, IntegerValue::Signed(-128)).unwrap(),
            negative_divisor_term,
        )
        .unwrap(),
    );
    let bounded_negative_divisor_upper_requirement = Proposition::LessOrEqual(
        bounded_negative_divisor_term,
        ScalarTerm::integer(signed_arithmetic_type, IntegerValue::Signed(-1)).unwrap(),
    );
    let add_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let runtime_multiply_requirement = Proposition::LessOrEqual(
        small_term,
        ScalarTerm::exact_integer_divide(
            add_type,
            ScalarTerm::integer(add_type, IntegerValue::Unsigned(255)).unwrap(),
            divisor_term,
        )
        .unwrap(),
    );
    let runtime_add_requirement = Proposition::LessOrEqual(
        add_left_term,
        ScalarTerm::exact_integer_subtract(
            add_type,
            ScalarTerm::integer(add_type, IntegerValue::Unsigned(255)).unwrap(),
            add_right_term,
        )
        .unwrap(),
    );
    let positive_addend_sign_requirement = Proposition::LessOrEqual(
        ScalarTerm::integer(signed_arithmetic_type, IntegerValue::Signed(0)).unwrap(),
        positive_addend_term.clone(),
    );
    let runtime_positive_add_requirement = Proposition::LessOrEqual(
        ScalarTerm::value(entry.parameters[5].id, entry.parameters[5].scalar_type),
        ScalarTerm::exact_integer_subtract(
            signed_arithmetic_type,
            ScalarTerm::integer(signed_arithmetic_type, IntegerValue::Signed(127)).unwrap(),
            positive_addend_term,
        )
        .unwrap(),
    );
    let negative_addend_sign_requirement = Proposition::LessOrEqual(
        negative_addend_term.clone(),
        ScalarTerm::integer(signed_arithmetic_type, IntegerValue::Signed(0)).unwrap(),
    );
    let runtime_negative_add_requirement = Proposition::LessOrEqual(
        ScalarTerm::exact_integer_subtract(
            signed_arithmetic_type,
            ScalarTerm::integer(signed_arithmetic_type, IntegerValue::Signed(-128)).unwrap(),
            negative_addend_term,
        )
        .unwrap(),
        ScalarTerm::value(entry.parameters[5].id, entry.parameters[5].scalar_type),
    );
    let positive_subtrahend_sign_requirement = Proposition::LessOrEqual(
        ScalarTerm::integer(signed_arithmetic_type, IntegerValue::Signed(0)).unwrap(),
        positive_subtrahend_term.clone(),
    );
    let runtime_positive_subtract_requirement = Proposition::LessOrEqual(
        ScalarTerm::exact_integer_add(
            signed_arithmetic_type,
            ScalarTerm::integer(signed_arithmetic_type, IntegerValue::Signed(-128)).unwrap(),
            positive_subtrahend_term,
        )
        .unwrap(),
        ScalarTerm::value(entry.parameters[5].id, entry.parameters[5].scalar_type),
    );
    let negative_subtrahend_sign_requirement = Proposition::LessOrEqual(
        negative_subtrahend_term.clone(),
        ScalarTerm::integer(signed_arithmetic_type, IntegerValue::Signed(0)).unwrap(),
    );
    let runtime_negative_subtract_requirement = Proposition::LessOrEqual(
        ScalarTerm::value(entry.parameters[5].id, entry.parameters[5].scalar_type),
        ScalarTerm::exact_integer_add(
            signed_arithmetic_type,
            ScalarTerm::integer(signed_arithmetic_type, IntegerValue::Signed(127)).unwrap(),
            negative_subtrahend_term,
        )
        .unwrap(),
    );
    let runtime_signed_shift_count_lower_requirement = Proposition::LessOrEqual(
        ScalarTerm::integer(signed_arithmetic_type, IntegerValue::Signed(0)).unwrap(),
        signed_count_term.clone(),
    );
    let runtime_signed_shift_count_upper_requirement = Proposition::LessOrEqual(
        signed_count_term,
        ScalarTerm::integer(signed_arithmetic_type, IntegerValue::Signed(2)).unwrap(),
    );
    for requirement in [
        &input_upper_requirement,
        &shift_upper_requirement,
        &exact_upper_requirement,
        &left_shift_value_requirement,
        &add_upper_requirement,
        &bitwise_not_exact_add_requirement,
        &widen_exact_subtract_requirement,
        &divisor_lower_requirement,
        &runtime_subtract_requirement,
        &runtime_multiply_requirement,
        &left_shift_count_requirement,
        &signed_lower_requirement,
        &signed_upper_requirement,
        &signed_arithmetic_lower_requirement,
        &signed_arithmetic_upper_requirement,
        &signed_multiply_lower_requirement,
        &signed_multiply_upper_requirement,
        &signed_shift_value_lower_requirement,
        &signed_shift_value_upper_requirement,
        &signed_nonnegative_requirement,
        &signed_shift_count_lower_requirement,
        &signed_shift_count_upper_requirement,
        &signed_divisor_lower_requirement,
        &runtime_signed_positive_multiply_lower_requirement,
        &runtime_signed_positive_multiply_upper_requirement,
        &negative_divisor_upper_requirement,
        &runtime_signed_negative_multiply_lower_requirement,
        &runtime_signed_negative_multiply_upper_requirement,
        &bounded_negative_divisor_upper_requirement,
        &runtime_add_requirement,
        &positive_addend_sign_requirement,
        &runtime_positive_add_requirement,
        &negative_addend_sign_requirement,
        &runtime_negative_add_requirement,
        &positive_subtrahend_sign_requirement,
        &runtime_positive_subtract_requirement,
        &negative_subtrahend_sign_requirement,
        &runtime_negative_subtract_requirement,
        &runtime_signed_shift_count_lower_requirement,
        &runtime_signed_shift_count_upper_requirement,
    ] {
        assert!(entry.contract.requires.contains(requirement));
    }
    assert!(entry.blocks.iter().any(|block| {
        block
            .operations
            .iter()
            .any(|operation| matches!(operation.kind, OperationKind::IntegerLessThan { .. }))
    }));
    assert!(entry.blocks.iter().any(|block| {
        block
            .operations
            .iter()
            .any(|operation| matches!(operation.kind, OperationKind::WrappingIntegerAdd { .. }))
    }));
    assert!(entry.blocks.iter().any(|block| {
        block
            .operations
            .iter()
            .any(|operation| matches!(operation.kind, OperationKind::IntegerBitwiseNot { .. }))
    }));
    assert!(entry.blocks.iter().any(|block| {
        block
            .operations
            .iter()
            .any(|operation| matches!(operation.kind, OperationKind::IntegerWiden { .. }))
    }));
    let cast_obligation = entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match operation.kind {
            OperationKind::IntegerExactCast { obligation, .. } => Some(obligation),
            _ => None,
        })
        .expect("shared convergence retains the guarded exact cast");
    assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
        evidence.obligation == cast_obligation
            && matches!(
                evidence.route,
                proof_admission::EvidenceRoute::CertificateDerived(_)
            )
    }));
    let signed_parameter = entry.parameters[4].id;
    let signed_cast_obligation = entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match operation.kind {
            OperationKind::IntegerExactCast {
                operand,
                obligation,
            } if operand == signed_parameter => Some(obligation),
            _ => None,
        })
        .expect("shared convergence retains the signed guarded exact cast");
    assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
        evidence.obligation == signed_cast_obligation
            && matches!(
                evidence.route,
                proof_admission::EvidenceRoute::CertificateDerived(_)
            )
    }));
    let cross_sign_cast_obligations = [entry.parameters[1].id, entry.parameters[5].id]
        .into_iter()
        .map(|parameter| {
            entry
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .find_map(|operation| match operation.kind {
                    OperationKind::IntegerExactCast {
                        operand,
                        obligation,
                    } if operand == parameter => Some(obligation),
                    _ => None,
                })
                .expect("shared convergence retains each cross-sign guarded exact cast")
        })
        .collect::<Vec<_>>();
    for obligation in &cross_sign_cast_obligations {
        assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
            evidence.obligation == *obligation
                && matches!(
                    evidence.route,
                    proof_admission::EvidenceRoute::CertificateDerived(_)
                )
        }));
    }
    let roundtrip_cast_obligation = entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| {
            let OperationKind::IntegerExactCast {
                operand,
                obligation,
            } = operation.kind
            else {
                return None;
            };
            entry
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .find(|candidate| {
                    candidate.result.scalar_ref().map(|result| result.id) == Some(operand)
                        && matches!(
                            candidate.kind,
                            OperationKind::IntegerWiden { operand }
                                if operand == entry.parameters[1].id
                        )
                })
                .map(|_| obligation)
        })
        .expect("shared convergence retains the direct widen-then-narrow exact cast");
    assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
        evidence.obligation == roundtrip_cast_obligation
            && matches!(
                evidence.route,
                proof_admission::EvidenceRoute::CertificateDerived(_)
            )
    }));
    let signed_arithmetic_parameter = entry.parameters[5].id;
    let signed_add_sites = entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match operation.kind {
            OperationKind::ExactIntegerAdd {
                left,
                right,
                obligation,
            } if left == signed_arithmetic_parameter => entry
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .find_map(|candidate| {
                    (candidate.result.scalar_ref().map(|result| result.id) == Some(right))
                        .then_some(match candidate.kind {
                            OperationKind::IntegerConstant { value } => Some(value),
                            _ => None,
                        })
                        .flatten()
                })
                .map(|addend| (obligation, addend)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(
        signed_add_sites
            .iter()
            .any(|(_, addend)| *addend == IntegerValue::Signed(1))
    );
    assert!(
        signed_add_sites
            .iter()
            .any(|(_, addend)| *addend == IntegerValue::Signed(-1))
    );
    for (obligation, _) in &signed_add_sites {
        assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
            evidence.obligation == *obligation
                && matches!(
                    evidence.route,
                    proof_admission::EvidenceRoute::CertificateDerived(_)
                )
        }));
    }
    let signed_subtract_sites = entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match operation.kind {
            OperationKind::ExactIntegerSubtract {
                left,
                right,
                obligation,
            } if left == signed_arithmetic_parameter => entry
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .find_map(|candidate| {
                    (candidate.result.scalar_ref().map(|result| result.id) == Some(right))
                        .then_some(match candidate.kind {
                            OperationKind::IntegerConstant { value } => Some(value),
                            _ => None,
                        })
                        .flatten()
                })
                .map(|subtrahend| (obligation, subtrahend)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(
        signed_subtract_sites
            .iter()
            .any(|(_, subtrahend)| *subtrahend == IntegerValue::Signed(1))
    );
    assert!(
        signed_subtract_sites
            .iter()
            .any(|(_, subtrahend)| *subtrahend == IntegerValue::Signed(-1))
    );
    for (obligation, _) in &signed_subtract_sites {
        assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
            evidence.obligation == *obligation
                && matches!(
                    evidence.route,
                    proof_admission::EvidenceRoute::CertificateDerived(_)
                )
        }));
    }
    let signed_multiply_sites = entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match operation.kind {
            OperationKind::ExactIntegerMultiply {
                left,
                right,
                obligation,
            } if left == signed_arithmetic_parameter => entry
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .find_map(|candidate| {
                    (candidate.result.scalar_ref().map(|result| result.id) == Some(right))
                        .then_some(match candidate.kind {
                            OperationKind::IntegerConstant { value } => Some(value),
                            _ => None,
                        })
                        .flatten()
                })
                .map(|factor| (obligation, factor)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(
        signed_multiply_sites
            .iter()
            .any(|(_, factor)| *factor == IntegerValue::Signed(3))
    );
    assert!(
        signed_multiply_sites
            .iter()
            .any(|(_, factor)| *factor == IntegerValue::Signed(-3))
    );
    for (obligation, _) in &signed_multiply_sites {
        assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
            evidence.obligation == *obligation
                && matches!(
                    evidence.route,
                    proof_admission::EvidenceRoute::CertificateDerived(_)
                )
        }));
    }
    let signed_division_obligations = entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match operation.kind {
            OperationKind::ExactIntegerDivide {
                left, obligation, ..
            }
            | OperationKind::ExactIntegerRemainder {
                left, obligation, ..
            } if left == signed_arithmetic_parameter => Some(obligation),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(entry.blocks.iter().any(|block| block.operations.iter().any(
        |operation| matches!(operation.kind, OperationKind::ExactIntegerDivide { left, .. }
            if left == signed_arithmetic_parameter)
    )));
    assert!(entry.blocks.iter().any(|block| block.operations.iter().any(
        |operation| matches!(operation.kind, OperationKind::ExactIntegerRemainder { left, .. }
            if left == signed_arithmetic_parameter)
    )));
    assert!(signed_division_obligations.len() >= 2);
    for obligation in &signed_division_obligations {
        assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
            evidence.obligation == *obligation
                && matches!(
                    evidence.route,
                    proof_admission::EvidenceRoute::CertificateDerived(_)
                )
        }));
    }
    let carrier_total_literal_division_obligations = entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| {
            let (right, obligation, is_remainder) = match operation.kind {
                OperationKind::ExactIntegerDivide {
                    right, obligation, ..
                } => (right, obligation, false),
                OperationKind::ExactIntegerRemainder {
                    right, obligation, ..
                } => (right, obligation, true),
                _ => return None,
            };
            entry
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .find_map(|candidate| {
                    (candidate.result.scalar_ref().map(|result| result.id) == Some(right))
                        .then_some(match candidate.kind {
                            OperationKind::IntegerConstant { value }
                                if matches!(
                                    value,
                                    IntegerValue::Unsigned(value) if value != 0
                                ) || matches!(
                                    value,
                                    IntegerValue::Signed(value) if value != 0 && value != -1
                                ) => Some(value),
                            _ => None,
                        })
                        .flatten()
                })
                .map(|literal| (obligation, literal, is_remainder))
        })
        .collect::<Vec<_>>();
    assert!(carrier_total_literal_division_obligations.iter().any(
        |(_, literal, _)| matches!(literal, IntegerValue::Unsigned(_))
    ));
    assert!(carrier_total_literal_division_obligations.iter().any(
        |(_, literal, _)| matches!(literal, IntegerValue::Signed(_))
    ));
    assert!(carrier_total_literal_division_obligations.iter().any(
        |(_, _, is_remainder)| !is_remainder
    ));
    assert!(carrier_total_literal_division_obligations.iter().any(
        |(_, _, is_remainder)| *is_remainder
    ));
    let reconstructed = terminal_verifier::reconstruct_operation_obligations(
        &lowered.semantic_module,
    )
    .expect("reconstruct literal division obligations");
    for (obligation, literal, _) in &carrier_total_literal_division_obligations {
        let evidence = lowered
            .proof_bundle
            .evidence
            .iter()
            .find(|evidence| evidence.obligation == *obligation)
            .expect("carrier-total literal exact operation has evidence");
        let EvidenceRoute::CertificateDerived(certificate) = &evidence.route else {
            panic!("carrier-total literal exact operation has a recursive certificate")
        };
        assert!(matches!(
            literal,
            IntegerValue::Unsigned(_)
                | IntegerValue::Signed(_)
        ));
        let site = reconstructed
            .iter()
            .find(|site| site.obligation.id == *obligation)
            .expect("reconstructed carrier-total literal division site");
        assert!(site.canonical_certificate);
        assert_eq!(certificate.proof.conclusion, site.obligation.proposition);
    }
    let retained_bound_negative_one_obligations = entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| {
            let (left, right, obligation, is_remainder) = match operation.kind {
                OperationKind::ExactIntegerDivide {
                    left, right, obligation, ..
                } => (left, right, obligation, false),
                OperationKind::ExactIntegerRemainder {
                    left, right, obligation, ..
                } => (left, right, obligation, true),
                _ => return None,
            };
            if left != signed_arithmetic_parameter {
                return None;
            }
            entry
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .any(|candidate| {
                    candidate.result.scalar_ref().map(|result| result.id) == Some(right)
                        && matches!(
                            candidate.kind,
                            OperationKind::IntegerConstant {
                                value: IntegerValue::Signed(-1),
                            }
                        )
                })
                .then_some((obligation, is_remainder))
        })
        .collect::<Vec<_>>();
    assert!(retained_bound_negative_one_obligations.len() >= 2);
    assert!(retained_bound_negative_one_obligations.iter().any(|(_, is_remainder)| !is_remainder));
    assert!(retained_bound_negative_one_obligations.iter().any(|(_, is_remainder)| *is_remainder));
    for (obligation, _) in retained_bound_negative_one_obligations {
        let evidence = lowered
            .proof_bundle
            .evidence
            .iter()
            .find(|evidence| evidence.obligation == obligation)
            .expect("retained-bound -1 exact operation has evidence");
        let EvidenceRoute::CertificateDerived(certificate) = &evidence.route else {
            panic!("retained-bound -1 exact operation has a recursive certificate")
        };
        let ProofRule::DisjunctionIntroduction { disjunct, index } = &certificate.proof.rule else {
            panic!("retained-bound -1 operation selects the exact exceptional arm")
        };
        assert_eq!(*index, 2);
        let ProofRule::ConjunctionIntroduction(conjuncts) = &disjunct.rule else {
            panic!("retained-bound -1 operation proves both exceptional premises")
        };
        assert!(matches!(
            conjuncts[0].rule,
            ProofRule::IntegerLessOrEqualSubstitution { .. }
        ));
        assert!(matches!(
            conjuncts[1].rule,
            ProofRule::Assumption { .. }
        ));
    }
    let transitive_bound_negative_one_obligations = entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| {
            let (left, right, obligation, is_remainder) = match operation.kind {
                OperationKind::ExactIntegerDivide {
                    left, right, obligation, ..
                } => (left, right, obligation, false),
                OperationKind::ExactIntegerRemainder {
                    left, right, obligation, ..
                } => (left, right, obligation, true),
                _ => return None,
            };
            if left != signed_parameter {
                return None;
            }
            entry
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .any(|candidate| {
                    candidate.result.scalar_ref().map(|result| result.id) == Some(right)
                        && matches!(
                            candidate.kind,
                            OperationKind::IntegerConstant {
                                value: IntegerValue::Signed(-1),
                            }
                        )
                })
                .then_some((obligation, is_remainder))
        })
        .collect::<Vec<_>>();
    assert!(transitive_bound_negative_one_obligations.len() >= 2);
    assert!(transitive_bound_negative_one_obligations.iter().any(|(_, is_remainder)| !is_remainder));
    assert!(transitive_bound_negative_one_obligations.iter().any(|(_, is_remainder)| *is_remainder));
    for (obligation, _) in &transitive_bound_negative_one_obligations {
        let evidence = lowered
            .proof_bundle
            .evidence
            .iter()
            .find(|evidence| evidence.obligation == *obligation)
            .expect("transitive-bound -1 exact operation has evidence");
        let EvidenceRoute::CertificateDerived(certificate) = &evidence.route else {
            panic!("transitive-bound -1 exact operation has a recursive certificate")
        };
        let ProofRule::DisjunctionIntroduction { disjunct, index } = &certificate.proof.rule else {
            panic!("transitive-bound -1 operation selects the exact exceptional arm")
        };
        assert_eq!(*index, 2);
        let ProofRule::ConjunctionIntroduction(conjuncts) = &disjunct.rule else {
            panic!("transitive-bound -1 operation proves both exceptional premises")
        };
        assert!(matches!(
            conjuncts[1].rule,
            ProofRule::IntegerLessOrEqualTransitivity { .. }
        ));
    }
    let exact_divide_obligation = entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match operation.kind {
            OperationKind::ExactIntegerDivide { obligation, .. } => Some(obligation),
            _ => None,
        })
        .expect("shared convergence retains exact division by a nonzero constant");
    assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
        evidence.obligation == exact_divide_obligation
            && matches!(
                evidence.route,
                proof_admission::EvidenceRoute::CertificateDerived(_)
            )
    }));
    let exact_remainder_obligation = entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match operation.kind {
            OperationKind::ExactIntegerRemainder { obligation, .. } => Some(obligation),
            _ => None,
        })
        .expect("shared convergence retains exact remainder by a nonzero constant");
    assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
        evidence.obligation == exact_remainder_obligation
            && matches!(
                evidence.route,
                proof_admission::EvidenceRoute::CertificateDerived(_)
            )
    }));
    let divisor_parameter = entry.parameters[2].id;
    let runtime_exact_divide_obligation = entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match operation.kind {
            OperationKind::ExactIntegerDivide {
                right, obligation, ..
            } if right == divisor_parameter => Some(obligation),
            _ => None,
        })
        .expect("shared convergence retains exact division by a proven runtime divisor");
    let runtime_exact_remainder_obligation = entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match operation.kind {
            OperationKind::ExactIntegerRemainder {
                right, obligation, ..
            } if right == divisor_parameter => Some(obligation),
            _ => None,
        })
        .expect("shared convergence retains exact remainder by a proven runtime divisor");
    for obligation in [
        runtime_exact_divide_obligation,
        runtime_exact_remainder_obligation,
    ] {
        let evidence = lowered
            .proof_bundle
            .evidence
            .iter()
            .find(|evidence| evidence.obligation == obligation)
            .expect("unsigned runtime-divisor exact operation has evidence");
        let EvidenceRoute::CertificateDerived(certificate) = &evidence.route else {
            panic!("unsigned runtime-divisor exact operation has a recursive certificate")
        };
        assert!(matches!(
            certificate.proof.conclusion,
            Proposition::LessOrEqual(_, _)
        ));
        assert!(matches!(
            certificate.proof.rule,
            ProofRule::Assumption { .. }
        ));
    }
    let signed_divisor_parameter = entry.parameters[6].id;
    let runtime_signed_division_obligations = entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match operation.kind {
            OperationKind::ExactIntegerDivide {
                right, obligation, ..
            }
            | OperationKind::ExactIntegerRemainder {
                right, obligation, ..
            } if right == signed_divisor_parameter => Some(obligation),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(runtime_signed_division_obligations.len() >= 2);
    for obligation in &runtime_signed_division_obligations {
        let evidence = lowered
            .proof_bundle
            .evidence
            .iter()
            .find(|evidence| evidence.obligation == *obligation)
            .expect("signed-positive runtime-divisor exact operation has evidence");
        let EvidenceRoute::CertificateDerived(certificate) = &evidence.route else {
            panic!("signed-positive runtime-divisor exact operation has a recursive certificate")
        };
        let ProofRule::DisjunctionIntroduction { disjunct, index } = &certificate.proof.rule else {
            panic!("signed-positive runtime divisor selects its canonical arm")
        };
        assert_eq!(*index, 1);
        assert!(matches!(disjunct.rule, ProofRule::Assumption { .. }));
    }
    let negative_divisor_parameter = entry.parameters[7].id;
    let runtime_negative_signed_division_obligations = entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match operation.kind {
            OperationKind::ExactIntegerDivide {
                right, obligation, ..
            }
            | OperationKind::ExactIntegerRemainder {
                right, obligation, ..
            } if right == negative_divisor_parameter => Some(obligation),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(runtime_negative_signed_division_obligations.len() >= 2);
    for obligation in &runtime_negative_signed_division_obligations {
        let evidence = lowered
            .proof_bundle
            .evidence
            .iter()
            .find(|evidence| evidence.obligation == *obligation)
            .expect("signed-negative runtime-divisor exact operation has evidence");
        let EvidenceRoute::CertificateDerived(certificate) = &evidence.route else {
            panic!("signed-negative runtime-divisor exact operation has a recursive certificate")
        };
        let ProofRule::DisjunctionIntroduction { disjunct, index } = &certificate.proof.rule else {
            panic!("signed-negative runtime divisor selects its canonical arm")
        };
        assert_eq!(*index, 0);
        assert!(matches!(disjunct.rule, ProofRule::Assumption { .. }));
    }
    let bounded_negative_divisor_parameter = entry.parameters[8].id;
    let runtime_bounded_negative_signed_division_obligations = entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match operation.kind {
            OperationKind::ExactIntegerDivide {
                right, obligation, ..
            }
            | OperationKind::ExactIntegerRemainder {
                right, obligation, ..
            } if right == bounded_negative_divisor_parameter => Some(obligation),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(runtime_bounded_negative_signed_division_obligations.len() >= 2);
    for obligation in &runtime_bounded_negative_signed_division_obligations {
        assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
            evidence.obligation == *obligation
                && matches!(
                    evidence.route,
                    proof_admission::EvidenceRoute::CertificateDerived(_)
                )
        }));
    }
    let runtime_exact_add_obligation = entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match operation.kind {
            OperationKind::ExactIntegerAdd {
                left,
                right,
                obligation,
            } if left == entry.parameters[9].id && right == entry.parameters[10].id => {
                Some(obligation)
            }
            _ => None,
        })
        .expect("shared convergence retains the computed-bound runtime addition");
    assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
        evidence.obligation == runtime_exact_add_obligation
            && matches!(
                evidence.route,
                proof_admission::EvidenceRoute::CertificateDerived(_)
            )
    }));
    let runtime_signed_add_obligations = [entry.parameters[11].id, entry.parameters[12].id]
        .into_iter()
        .map(|addend| {
            entry
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .find_map(|operation| match operation.kind {
                    OperationKind::ExactIntegerAdd {
                        left,
                        right,
                        obligation,
                    } if left == entry.parameters[5].id && right == addend => Some(obligation),
                    _ => None,
                })
                .expect("shared convergence retains each signed computed-bound runtime addition")
        })
        .collect::<Vec<_>>();
    for obligation in &runtime_signed_add_obligations {
        assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
            evidence.obligation == *obligation
                && matches!(
                    evidence.route,
                    proof_admission::EvidenceRoute::CertificateDerived(_)
                )
        }));
    }
    let runtime_signed_subtract_obligations = [entry.parameters[13].id, entry.parameters[14].id]
        .into_iter()
        .map(|subtrahend| {
            entry
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .find_map(|operation| match operation.kind {
                    OperationKind::ExactIntegerSubtract {
                        left,
                        right,
                        obligation,
                    } if left == entry.parameters[5].id && right == subtrahend => Some(obligation),
                    _ => None,
                })
                .expect("shared convergence retains each signed computed-bound runtime subtraction")
        })
        .collect::<Vec<_>>();
    for obligation in &runtime_signed_subtract_obligations {
        assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
            evidence.obligation == *obligation
                && matches!(
                    evidence.route,
                    proof_admission::EvidenceRoute::CertificateDerived(_)
                )
        }));
    }
    let exact_shift_obligation = entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match operation.kind {
            OperationKind::ExactIntegerShiftRight { obligation, .. } => Some(obligation),
            _ => None,
        })
        .expect("shared convergence retains the bounded exact right shift");
    assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
        evidence.obligation == exact_shift_obligation
            && matches!(
                evidence.route,
                proof_admission::EvidenceRoute::CertificateDerived(_)
            )
    }));
    let signed_count_exact_shift_obligation = entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match operation.kind {
            OperationKind::ExactIntegerShiftRight {
                value,
                count,
                obligation,
            } if value == entry.parameters[5].id && count == entry.parameters[6].id => {
                Some(obligation)
            }
            _ => None,
        })
        .expect("shared convergence retains the signed-count exact right shift");
    assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
        evidence.obligation == signed_count_exact_shift_obligation
            && matches!(
                evidence.route,
                proof_admission::EvidenceRoute::CertificateDerived(_)
            )
    }));
    let exact_shift_left_obligation = entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match operation.kind {
            OperationKind::ExactIntegerShiftLeft { obligation, .. } => Some(obligation),
            _ => None,
        })
        .expect("shared convergence retains the bounded exact left shift");
    assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
        evidence.obligation == exact_shift_left_obligation
            && matches!(
                evidence.route,
                proof_admission::EvidenceRoute::CertificateDerived(_)
            )
    }));
    let count_parameter = entry.parameters[3].id;
    let runtime_exact_shift_left_obligation = entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match operation.kind {
            OperationKind::ExactIntegerShiftLeft {
                count, obligation, ..
            } if count == count_parameter => Some(obligation),
            _ => None,
        })
        .expect("shared convergence retains the proven runtime exact left shift");
    assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
        evidence.obligation == runtime_exact_shift_left_obligation
            && matches!(
                evidence.route,
                proof_admission::EvidenceRoute::CertificateDerived(_)
            )
    }));
    let runtime_signed_count_shift_left_obligation = entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match operation.kind {
            OperationKind::ExactIntegerShiftLeft {
                value,
                count,
                obligation,
            } if value == entry.parameters[1].id && count == entry.parameters[15].id => {
                Some(obligation)
            }
            _ => None,
        })
        .expect("shared convergence retains the signed-count runtime exact left shift");
    assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
        evidence.obligation == runtime_signed_count_shift_left_obligation
            && matches!(
                evidence.route,
                proof_admission::EvidenceRoute::CertificateDerived(_)
            )
    }));
    let signed_value_shift_left_obligations = entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match operation.kind {
            OperationKind::ExactIntegerShiftLeft {
                value, obligation, ..
            } if value == entry.parameters[5].id => Some(obligation),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(signed_value_shift_left_obligations.len() >= 3);
    for obligation in &signed_value_shift_left_obligations {
        assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
            evidence.obligation == *obligation
                && matches!(
                    evidence.route,
                    proof_admission::EvidenceRoute::CertificateDerived(_)
                )
        }));
    }
    let exact_multiply_obligation = entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match operation.kind {
            OperationKind::ExactIntegerMultiply { obligation, .. } => Some(obligation),
            _ => None,
        })
        .expect("shared convergence retains the bounded exact multiplication");
    assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
        evidence.obligation == exact_multiply_obligation
            && matches!(
                evidence.route,
                proof_admission::EvidenceRoute::CertificateDerived(_)
            )
    }));
    let runtime_exact_multiply_obligation = entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match operation.kind {
            OperationKind::ExactIntegerMultiply {
                left,
                right,
                obligation,
            } if left == entry.parameters[1].id && right == entry.parameters[2].id => {
                Some(obligation)
            }
            _ => None,
        })
        .expect("shared convergence retains the computed-bound runtime multiplication");
    assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
        evidence.obligation == runtime_exact_multiply_obligation
            && matches!(
                evidence.route,
                proof_admission::EvidenceRoute::CertificateDerived(_)
            )
    }));
    let runtime_signed_multiply_obligations = [entry.parameters[6].id, entry.parameters[7].id]
        .into_iter()
        .map(|factor| {
            entry
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .find_map(|operation| match operation.kind {
                    OperationKind::ExactIntegerMultiply {
                        left,
                        right,
                        obligation,
                    } if left == entry.parameters[5].id && right == factor => Some(obligation),
                    _ => None,
                })
                .expect("shared convergence retains each signed quotient-bound multiplication")
        })
        .collect::<Vec<_>>();
    for obligation in &runtime_signed_multiply_obligations {
        assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
            evidence.obligation == *obligation
                && matches!(
                    evidence.route,
                    proof_admission::EvidenceRoute::CertificateDerived(_)
                )
        }));
    }
    let exact_subtract_obligation = entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match operation.kind {
            OperationKind::ExactIntegerSubtract {
                left,
                right,
                obligation,
            } if right == entry.parameters[1].id => entry
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .find_map(|candidate| {
                    (candidate.result.scalar_ref().map(|result| result.id) == Some(left))
                        .then_some(match candidate.kind {
                            OperationKind::IntegerConstant { value } => Some(value),
                            _ => None,
                        })
                        .flatten()
                })
                .filter(|value| *value == IntegerValue::Unsigned(127))
                .map(|_| obligation),
            _ => None,
        })
        .expect("shared convergence retains the bounded exact subtraction");
    assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
        evidence.obligation == exact_subtract_obligation
            && matches!(
                evidence.route,
                proof_admission::EvidenceRoute::CertificateDerived(_)
            )
    }));
    let runtime_exact_subtract_obligation = entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match operation.kind {
            OperationKind::ExactIntegerSubtract {
                left,
                right,
                obligation,
            } if left == entry.parameters[1].id && right == entry.parameters[2].id => {
                Some(obligation)
            }
            _ => None,
        })
        .expect("shared convergence retains the relationally proven runtime subtraction");
    assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
        evidence.obligation == runtime_exact_subtract_obligation
            && matches!(
                evidence.route,
                proof_admission::EvidenceRoute::CertificateDerived(_)
            )
    }));
    let exact_add_obligation = entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match operation.kind {
            OperationKind::ExactIntegerAdd { obligation, .. } => Some(obligation),
            _ => None,
        })
        .expect("shared convergence retains the proven exact addition");
    assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
        evidence.obligation == exact_add_obligation
            && matches!(
                evidence.route,
                proof_admission::EvidenceRoute::CertificateDerived(_)
            )
    }));
    let operations = entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .collect::<Vec<_>>();
    let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let is_u8_one = |value| {
        operations.iter().any(|operation| {
            operation.result.scalar_ref().map(|result| result.id) == Some(value)
                && matches!(
                    operation.kind,
                    OperationKind::IntegerConstant {
                        value: IntegerValue::Unsigned(1)
                    }
                )
        })
    };
    let is_u8_two = |value| {
        operations.iter().any(|operation| {
            operation.result.scalar_ref().map(|result| result.id) == Some(value)
                && matches!(
                    operation.kind,
                    OperationKind::IntegerConstant {
                        value: IntegerValue::Unsigned(2)
                    }
                )
        })
    };
    let is_u8_three = |value| {
        operations.iter().any(|operation| {
            operation.result.scalar_ref().map(|result| result.id) == Some(value)
                && matches!(
                    operation.kind,
                    OperationKind::IntegerConstant {
                        value: IntegerValue::Unsigned(3)
                    }
                )
        })
    };
    let is_integer_constant = |value, integer_type, expected| {
        operations.iter().any(|operation| {
            operation.result.scalar_ref().is_some_and(|result| {
                result.id == value && result.scalar_type == ScalarType::Integer(integer_type)
            }) && matches!(
                operation.kind,
                OperationKind::IntegerConstant { value } if value == expected
            )
        })
    };
    let (nested_add_obligations, middle_addend, outer_addend) = operations
        .iter()
        .find_map(|outer| {
            let OperationKind::ExactIntegerAdd {
                left,
                right,
                obligation: outer_obligation,
            } = outer.kind
            else {
                return None;
            };
            if !is_u8_one(right) {
                return None;
            }
            let middle = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(left)
            })?;
            let OperationKind::ExactIntegerAdd {
                left: middle_left,
                right: middle_right,
                obligation: middle_obligation,
            } = middle.kind
            else {
                return None;
            };
            if !is_u8_one(middle_right) {
                return None;
            }
            let inner = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(middle_left)
            })?;
            let OperationKind::ExactIntegerAdd {
                left: inner_left,
                right: inner_right,
                obligation: inner_obligation,
            } = inner.kind
            else {
                return None;
            };
            (inner_left == entry.parameters[1].id && is_u8_one(inner_right)).then_some((
                [inner_obligation, middle_obligation, outer_obligation],
                middle_right,
                right,
            ))
        })
        .expect("a finite three-operation exact-add chain is retained");
    assert_ne!(nested_add_obligations[0], nested_add_obligations[1]);
    assert_ne!(nested_add_obligations[1], nested_add_obligations[2]);
    assert_ne!(nested_add_obligations[0], nested_add_obligations[2]);
    for obligation in nested_add_obligations {
        assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
            evidence.obligation == obligation
                && matches!(
                    evidence.route,
                    proof_admission::EvidenceRoute::CertificateDerived(_)
                )
        }));
    }
    let (nested_multiply_obligations, middle_factor) = operations
        .iter()
        .find_map(|outer| {
            let OperationKind::ExactIntegerMultiply {
                left,
                right,
                obligation: outer_obligation,
            } = outer.kind
            else {
                return None;
            };
            if !is_u8_one(right) {
                return None;
            }
            let middle = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(left)
            })?;
            let OperationKind::ExactIntegerMultiply {
                left: middle_left,
                right: middle_right,
                obligation: middle_obligation,
            } = middle.kind
            else {
                return None;
            };
            if !is_u8_three(middle_right) {
                return None;
            }
            let inner = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(middle_left)
            })?;
            let OperationKind::ExactIntegerMultiply {
                left: inner_left,
                right: inner_right,
                obligation: inner_obligation,
            } = inner.kind
            else {
                return None;
            };
            (inner_left == entry.parameters[1].id && is_u8_two(inner_right)).then_some((
                [inner_obligation, middle_obligation, outer_obligation],
                middle_right,
            ))
        })
        .expect("a finite three-operation exact-multiply chain is retained");
    assert_ne!(
        nested_multiply_obligations[0],
        nested_multiply_obligations[1]
    );
    assert_ne!(
        nested_multiply_obligations[1],
        nested_multiply_obligations[2]
    );
    assert_ne!(
        nested_multiply_obligations[0],
        nested_multiply_obligations[2]
    );
    for obligation in nested_multiply_obligations {
        assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
            evidence.obligation == obligation
                && matches!(
                    evidence.route,
                    proof_admission::EvidenceRoute::CertificateDerived(_)
                )
        }));
    }
    let (affine_obligations, affine_factor) = operations
        .iter()
        .find_map(|outer| {
            let OperationKind::ExactIntegerSubtract {
                left,
                right,
                obligation: outer_obligation,
            } = outer.kind
            else {
                return None;
            };
            if !is_u8_one(right) {
                return None;
            }
            let middle = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(left)
            })?;
            let OperationKind::ExactIntegerMultiply {
                left: middle_left,
                right: middle_right,
                obligation: middle_obligation,
            } = middle.kind
            else {
                return None;
            };
            if !is_u8_two(middle_right) {
                return None;
            }
            let inner = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(middle_left)
            })?;
            let OperationKind::ExactIntegerAdd {
                left: inner_left,
                right: inner_right,
                obligation: inner_obligation,
            } = inner.kind
            else {
                return None;
            };
            (inner_left == entry.parameters[1].id && is_u8_three(inner_right)).then_some((
                [inner_obligation, middle_obligation, outer_obligation],
                middle_right,
            ))
        })
        .expect("one left-associated mixed exact-affine chain is retained");
    let zero_affine_obligations = operations
        .iter()
        .find_map(|outer| {
            let OperationKind::ExactIntegerAdd {
                left,
                right,
                obligation: outer_obligation,
            } = outer.kind
            else {
                return None;
            };
            if !is_integer_constant(right, add_type, IntegerValue::Unsigned(255)) {
                return None;
            }
            let middle = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(left)
            })?;
            let OperationKind::ExactIntegerMultiply {
                left: middle_left,
                right: middle_right,
                obligation: middle_obligation,
            } = middle.kind
            else {
                return None;
            };
            if !is_integer_constant(middle_right, add_type, IntegerValue::Unsigned(0)) {
                return None;
            }
            let inner = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(middle_left)
            })?;
            let OperationKind::ExactIntegerAdd {
                left: inner_left,
                right: inner_right,
                obligation: inner_obligation,
            } = inner.kind
            else {
                return None;
            };
            (inner_left == entry.parameters[1].id && is_u8_three(inner_right)).then_some([
                inner_obligation,
                middle_obligation,
                outer_obligation,
            ])
        })
        .expect("a later zero factor retains every earlier affine-prefix proof");
    let signed_affine_obligations = operations
        .iter()
        .find_map(|outer| {
            let OperationKind::ExactIntegerSubtract {
                left,
                right,
                obligation: outer_obligation,
            } = outer.kind
            else {
                return None;
            };
            if !is_integer_constant(right, i8_type, IntegerValue::Signed(-1)) {
                return None;
            }
            let middle = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(left)
            })?;
            let OperationKind::ExactIntegerMultiply {
                left: middle_left,
                right: middle_right,
                obligation: middle_obligation,
            } = middle.kind
            else {
                return None;
            };
            if !is_integer_constant(middle_right, i8_type, IntegerValue::Signed(2)) {
                return None;
            }
            let inner = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(middle_left)
            })?;
            let OperationKind::ExactIntegerAdd {
                left: inner_left,
                right: inner_right,
                obligation: inner_obligation,
            } = inner.kind
            else {
                return None;
            };
            (inner_left == entry.parameters[5].id
                && is_integer_constant(inner_right, i8_type, IntegerValue::Signed(-3)))
            .then_some([inner_obligation, middle_obligation, outer_obligation])
        })
        .expect("one signed mixed exact-affine chain is retained");
    for obligations in [
        affine_obligations.as_slice(),
        zero_affine_obligations.as_slice(),
        signed_affine_obligations.as_slice(),
    ] {
        for (index, obligation) in obligations.iter().enumerate() {
            for other in &obligations[index + 1..] {
                assert_ne!(obligation, other);
            }
            let operation = operations
                .iter()
                .find(|operation| {
                    matches!(operation.kind,
                        OperationKind::ExactIntegerAdd { obligation: candidate, .. }
                        | OperationKind::ExactIntegerSubtract { obligation: candidate, .. }
                        | OperationKind::ExactIntegerMultiply { obligation: candidate, .. }
                        if candidate == *obligation)
                })
                .expect("affine obligation retains its exact operation");
            assert_eq!(
                TerminalFuelSchedule::CURRENT.operation_units(&operation.kind),
                1
            );
            assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
                evidence.obligation == *obligation
                    && matches!(evidence.route, EvidenceRoute::CertificateDerived(_))
            }));
        }
    }
    let (affine_cast_obligations, affine_cast_factor) = operations
        .iter()
        .find_map(|cast| {
            let OperationKind::IntegerExactCast {
                operand,
                obligation: cast_obligation,
            } = cast.kind
            else {
                return None;
            };
            if cast.result.scalar_ref().map(|result| result.scalar_type)
                != Some(ScalarType::Integer(i8_type))
            {
                return None;
            }
            let outer = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(operand)
            })?;
            let OperationKind::ExactIntegerSubtract {
                left,
                right,
                obligation: outer_obligation,
            } = outer.kind
            else {
                return None;
            };
            if !is_u8_one(right) {
                return None;
            }
            let middle = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(left)
            })?;
            let OperationKind::ExactIntegerMultiply {
                left: middle_left,
                right: middle_right,
                obligation: middle_obligation,
            } = middle.kind
            else {
                return None;
            };
            if !is_u8_two(middle_right) {
                return None;
            }
            let inner = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(middle_left)
            })?;
            let OperationKind::ExactIntegerAdd {
                left: inner_left,
                right: inner_right,
                obligation: inner_obligation,
            } = inner.kind
            else {
                return None;
            };
            (inner_left == entry.parameters[1].id && is_u8_three(inner_right)).then_some((
                [
                    inner_obligation,
                    middle_obligation,
                    outer_obligation,
                    cast_obligation,
                ],
                middle_right,
            ))
        })
        .expect("one mixed exact-affine chain feeds a partial exact cast");
    let zero_affine_cast_obligations = operations
        .iter()
        .find_map(|cast| {
            let OperationKind::IntegerExactCast {
                operand,
                obligation: cast_obligation,
            } = cast.kind
            else {
                return None;
            };
            if cast.result.scalar_ref().map(|result| result.scalar_type)
                != Some(ScalarType::Integer(i8_type))
            {
                return None;
            }
            let outer = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(operand)
            })?;
            let OperationKind::ExactIntegerAdd {
                left,
                right,
                obligation: outer_obligation,
            } = outer.kind
            else {
                return None;
            };
            if !is_integer_constant(right, add_type, IntegerValue::Unsigned(127)) {
                return None;
            }
            let middle = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(left)
            })?;
            let OperationKind::ExactIntegerMultiply {
                left: middle_left,
                right: middle_right,
                obligation: middle_obligation,
            } = middle.kind
            else {
                return None;
            };
            if !is_integer_constant(middle_right, add_type, IntegerValue::Unsigned(0)) {
                return None;
            }
            let inner = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(middle_left)
            })?;
            let OperationKind::ExactIntegerAdd {
                left: inner_left,
                right: inner_right,
                obligation: inner_obligation,
            } = inner.kind
            else {
                return None;
            };
            (inner_left == entry.parameters[1].id && is_u8_three(inner_right)).then_some([
                inner_obligation,
                middle_obligation,
                outer_obligation,
                cast_obligation,
            ])
        })
        .expect("zero-collapse affine chain retains every prefix and cast proof");
    let signed_affine_cast_obligations = operations
        .iter()
        .find_map(|cast| {
            let OperationKind::IntegerExactCast {
                operand,
                obligation: cast_obligation,
            } = cast.kind
            else {
                return None;
            };
            if cast.result.scalar_ref().map(|result| result.scalar_type)
                != Some(ScalarType::Integer(add_type))
            {
                return None;
            }
            let outer = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(operand)
            })?;
            let OperationKind::ExactIntegerAdd {
                left,
                right,
                obligation: outer_obligation,
            } = outer.kind
            else {
                return None;
            };
            if !is_integer_constant(right, i8_type, IntegerValue::Signed(1)) {
                return None;
            }
            let middle = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(left)
            })?;
            let OperationKind::ExactIntegerMultiply {
                left: middle_left,
                right: middle_right,
                obligation: middle_obligation,
            } = middle.kind
            else {
                return None;
            };
            if !is_integer_constant(middle_right, i8_type, IntegerValue::Signed(2)) {
                return None;
            }
            let inner = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(middle_left)
            })?;
            let OperationKind::ExactIntegerSubtract {
                left: inner_left,
                right: inner_right,
                obligation: inner_obligation,
            } = inner.kind
            else {
                return None;
            };
            (inner_left == entry.parameters[5].id
                && is_integer_constant(inner_right, i8_type, IntegerValue::Signed(3)))
            .then_some([
                inner_obligation,
                middle_obligation,
                outer_obligation,
                cast_obligation,
            ])
        })
        .expect("one signed affine chain feeds a cross-sign exact cast");
    for obligations in [
        affine_cast_obligations.as_slice(),
        zero_affine_cast_obligations.as_slice(),
        signed_affine_cast_obligations.as_slice(),
    ] {
        for (index, obligation) in obligations.iter().enumerate() {
            for other in &obligations[index + 1..] {
                assert_ne!(obligation, other);
            }
            let operation = operations
                .iter()
                .find(|operation| {
                    matches!(operation.kind,
                        OperationKind::ExactIntegerAdd { obligation: candidate, .. }
                        | OperationKind::ExactIntegerSubtract { obligation: candidate, .. }
                        | OperationKind::ExactIntegerMultiply { obligation: candidate, .. }
                        | OperationKind::IntegerExactCast { obligation: candidate, .. }
                        if candidate == *obligation)
                })
                .expect("affine cast obligation retains its exact operation");
            assert_eq!(
                TerminalFuelSchedule::CURRENT.operation_units(&operation.kind),
                1
            );
            assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
                evidence.obligation == *obligation
                    && matches!(evidence.route, EvidenceRoute::CertificateDerived(_))
            }));
        }
    }
    let bitwise_not_exact_add_obligations = entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match operation.kind {
            OperationKind::ExactIntegerAdd {
                left,
                right,
                obligation,
            } if left == entry.parameters[1].id => entry
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .find_map(|candidate| {
                    (candidate.result.scalar_ref().map(|result| result.id) == Some(right))
                        .then_some(match candidate.kind {
                            OperationKind::IntegerConstant { value } => Some(value),
                            _ => None,
                        })
                        .flatten()
                })
                .filter(|value| *value == IntegerValue::Unsigned(3))
                .map(|_| obligation),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(!bitwise_not_exact_add_obligations.is_empty());
    for obligation in &bitwise_not_exact_add_obligations {
        assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
            evidence.obligation == *obligation
                && matches!(
                    evidence.route,
                    proof_admission::EvidenceRoute::CertificateDerived(_)
                )
        }));
    }
    let widen_exact_subtract_obligation = operations
        .iter()
        .find_map(|operation| match operation.kind {
            OperationKind::ExactIntegerSubtract {
                left,
                right,
                obligation,
            } if left == entry.parameters[1].id && is_u8_three(right) => Some(obligation),
            _ => None,
        })
        .expect("the existing widened direct exact-subtract leaf is retained");
    assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
        evidence.obligation == widen_exact_subtract_obligation
            && matches!(
                evidence.route,
                proof_admission::EvidenceRoute::CertificateDerived(_)
            )
    }));
    let (nested_subtract_obligations, middle_subtrahend) = operations
        .iter()
        .find_map(|outer| {
            let OperationKind::ExactIntegerSubtract {
                left,
                right,
                obligation: outer_obligation,
            } = outer.kind
            else {
                return None;
            };
            if !is_u8_one(right) {
                return None;
            }
            let middle = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(left)
            })?;
            let OperationKind::ExactIntegerSubtract {
                left: middle_left,
                right: middle_right,
                obligation: middle_obligation,
            } = middle.kind
            else {
                return None;
            };
            if !is_u8_one(middle_right) {
                return None;
            }
            let inner = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(middle_left)
            })?;
            let OperationKind::ExactIntegerSubtract {
                left: inner_left,
                right: inner_right,
                obligation: inner_obligation,
            } = inner.kind
            else {
                return None;
            };
            (inner_left == entry.parameters[1].id && is_u8_one(inner_right)).then_some((
                [inner_obligation, middle_obligation, outer_obligation],
                middle_right,
            ))
        })
        .expect("a finite three-operation exact-subtract chain is retained");
    assert_ne!(
        nested_subtract_obligations[0],
        nested_subtract_obligations[1]
    );
    assert_ne!(
        nested_subtract_obligations[1],
        nested_subtract_obligations[2]
    );
    assert_ne!(
        nested_subtract_obligations[0],
        nested_subtract_obligations[2]
    );
    for obligation in nested_subtract_obligations {
        assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
            evidence.obligation == obligation
                && matches!(
                    evidence.route,
                    proof_admission::EvidenceRoute::CertificateDerived(_)
                )
        }));
    }
    let (mixed_add_subtract_obligations, mixed_subtrahend) = operations
        .iter()
        .find_map(|outer| {
            let OperationKind::ExactIntegerAdd {
                left,
                right,
                obligation: outer_obligation,
            } = outer.kind
            else {
                return None;
            };
            if !is_u8_one(right) {
                return None;
            }
            let middle = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(left)
            })?;
            let OperationKind::ExactIntegerSubtract {
                left: middle_left,
                right: middle_right,
                obligation: middle_obligation,
            } = middle.kind
            else {
                return None;
            };
            if !is_u8_two(middle_right) {
                return None;
            }
            let inner = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(middle_left)
            })?;
            let OperationKind::ExactIntegerAdd {
                left: inner_left,
                right: inner_right,
                obligation: inner_obligation,
            } = inner.kind
            else {
                return None;
            };
            (inner_left == entry.parameters[1].id && is_u8_three(inner_right)).then_some((
                [inner_obligation, middle_obligation, outer_obligation],
                middle_right,
            ))
        })
        .expect("a finite left-associated mixed exact-add/subtract chain is retained");
    assert_ne!(
        mixed_add_subtract_obligations[0],
        mixed_add_subtract_obligations[1]
    );
    assert_ne!(
        mixed_add_subtract_obligations[1],
        mixed_add_subtract_obligations[2]
    );
    assert_ne!(
        mixed_add_subtract_obligations[0],
        mixed_add_subtract_obligations[2]
    );
    for obligation in mixed_add_subtract_obligations {
        let operation = operations
            .iter()
            .find(|operation| {
                matches!(
                    operation.kind,
                    OperationKind::ExactIntegerAdd {
                        obligation: candidate,
                        ..
                    } | OperationKind::ExactIntegerSubtract {
                        obligation: candidate,
                        ..
                    } if candidate == obligation
                )
            })
            .expect("mixed exact-add/subtract obligation retains its operation");
        assert_eq!(
            TerminalFuelSchedule::CURRENT.operation_units(&operation.kind),
            1
        );
        assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
            evidence.obligation == obligation
                && matches!(
                    evidence.route,
                    proof_admission::EvidenceRoute::CertificateDerived(_)
                )
        }));
    }
    let offset_cast_target = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let (offset_chain_cast_obligations, offset_chain_cast_subtrahend) = operations
        .iter()
        .find_map(|cast| {
            let OperationKind::IntegerExactCast {
                operand,
                obligation: cast_obligation,
            } = cast.kind
            else {
                return None;
            };
            if cast.result.scalar_ref().map(|result| result.scalar_type)
                != Some(ScalarType::Integer(offset_cast_target))
            {
                return None;
            }
            let outer = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(operand)
            })?;
            let OperationKind::ExactIntegerAdd {
                left,
                right,
                obligation: outer_obligation,
            } = outer.kind
            else {
                return None;
            };
            if !is_u8_one(right) {
                return None;
            }
            let middle = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(left)
            })?;
            let OperationKind::ExactIntegerSubtract {
                left: middle_left,
                right: middle_right,
                obligation: middle_obligation,
            } = middle.kind
            else {
                return None;
            };
            if !is_u8_two(middle_right) {
                return None;
            }
            let inner = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(middle_left)
            })?;
            let OperationKind::ExactIntegerAdd {
                left: inner_left,
                right: inner_right,
                obligation: inner_obligation,
            } = inner.kind
            else {
                return None;
            };
            (inner_left == entry.parameters[1].id && is_u8_three(inner_right)).then_some((
                [
                    inner_obligation,
                    middle_obligation,
                    outer_obligation,
                    cast_obligation,
                ],
                middle_right,
            ))
        })
        .expect("one exact narrowing retains its complete landed-literal offset chain");
    for obligation in offset_chain_cast_obligations {
        let operation = operations
            .iter()
            .find(|operation| {
                matches!(
                    operation.kind,
                    OperationKind::IntegerExactCast {
                        obligation: candidate,
                        ..
                    } | OperationKind::ExactIntegerAdd {
                        obligation: candidate,
                        ..
                    } | OperationKind::ExactIntegerSubtract {
                        obligation: candidate,
                        ..
                    } if candidate == obligation
                )
            })
            .expect("offset-chain cast obligation retains its exact operation");
        assert_eq!(
            TerminalFuelSchedule::CURRENT.operation_units(&operation.kind),
            1
        );
        assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
            evidence.obligation == obligation
                && matches!(
                    evidence.route,
                    proof_admission::EvidenceRoute::CertificateDerived(_)
                )
        }));
    }
    let find_cast_then_offset = |subtract: bool| {
        operations.iter().find_map(|outer| {
            let (left, right, arithmetic_obligation) = match outer.kind {
                OperationKind::ExactIntegerAdd {
                    left,
                    right,
                    obligation,
                } if !subtract => (left, right, obligation),
                OperationKind::ExactIntegerSubtract {
                    left,
                    right,
                    obligation,
                } if subtract => (left, right, obligation),
                _ => return None,
            };
            if !operations.iter().any(|operation| {
                operation.result.scalar_ref().map(|result| result.id) == Some(right)
                    && matches!(
                        operation.kind,
                        OperationKind::IntegerConstant {
                            value: IntegerValue::Unsigned(5)
                        }
                    )
                    && operation
                        .result
                        .scalar_ref()
                        .map(|result| result.scalar_type)
                        == Some(ScalarType::Integer(
                            IntegerType::new(IntegerSign::Unsigned, 8).expect("u8"),
                        ))
            }) {
                return None;
            }
            let cast = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(left)
            })?;
            let OperationKind::IntegerExactCast {
                operand,
                obligation: cast_obligation,
            } = cast.kind
            else {
                return None;
            };
            (operand == entry.parameters[0].id)
                .then_some(([cast_obligation, arithmetic_obligation], right))
        })
    };
    let (cast_then_add_obligations, cast_then_add_literal) = find_cast_then_offset(false)
        .expect("one direct exact cast feeds one landed-literal exact addition");
    let (cast_then_subtract_obligations, _) = find_cast_then_offset(true)
        .expect("one direct exact cast feeds one landed-literal exact subtraction");
    for obligations in [cast_then_add_obligations, cast_then_subtract_obligations] {
        assert_ne!(obligations[0], obligations[1]);
        for obligation in obligations {
            let operation = operations
                .iter()
                .find(|operation| {
                    matches!(
                        operation.kind,
                        OperationKind::IntegerExactCast {
                            obligation: candidate,
                            ..
                        } | OperationKind::ExactIntegerAdd {
                            obligation: candidate,
                            ..
                        } | OperationKind::ExactIntegerSubtract {
                            obligation: candidate,
                            ..
                        } if candidate == obligation
                    )
                })
                .expect("cast-then-offset obligation retains its exact operation");
            assert_eq!(
                TerminalFuelSchedule::CURRENT.operation_units(&operation.kind),
                1
            );
            assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
                evidence.obligation == obligation
                    && matches!(
                        evidence.route,
                        proof_admission::EvidenceRoute::CertificateDerived(_)
                    )
            }));
        }
    }
    let target_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let (finite_cast_then_offset_obligations, finite_middle_literal) = operations
        .iter()
        .find_map(|outer| {
            let OperationKind::ExactIntegerAdd {
                left,
                right,
                obligation: outer_obligation,
            } = outer.kind
            else {
                return None;
            };
            if !is_u8_two(right) {
                return None;
            }
            let middle = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(left)
            })?;
            let OperationKind::ExactIntegerSubtract {
                left: middle_left,
                right: middle_right,
                obligation: middle_obligation,
            } = middle.kind
            else {
                return None;
            };
            if !is_u8_three(middle_right) {
                return None;
            }
            let inner = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(middle_left)
            })?;
            let OperationKind::ExactIntegerAdd {
                left: inner_left,
                right: inner_right,
                obligation: inner_obligation,
            } = inner.kind
            else {
                return None;
            };
            if !is_integer_constant(inner_right, target_type, IntegerValue::Unsigned(5)) {
                return None;
            }
            let cast = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(inner_left)
            })?;
            let OperationKind::IntegerExactCast {
                operand,
                obligation: cast_obligation,
            } = cast.kind
            else {
                return None;
            };
            (operand == entry.parameters[0].id).then_some((
                [
                    cast_obligation,
                    inner_obligation,
                    middle_obligation,
                    outer_obligation,
                ],
                middle_right,
            ))
        })
        .expect("one direct exact cast roots a finite mixed landed-literal offset chain");
    let cancelling_cast_then_offset_obligations = operations
        .iter()
        .find_map(|outer| {
            let OperationKind::ExactIntegerSubtract {
                left,
                right,
                obligation: outer_obligation,
            } = outer.kind
            else {
                return None;
            };
            if !is_integer_constant(right, target_type, IntegerValue::Unsigned(5)) {
                return None;
            }
            let inner = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(left)
            })?;
            let OperationKind::ExactIntegerAdd {
                left: inner_left,
                right: inner_right,
                obligation: inner_obligation,
            } = inner.kind
            else {
                return None;
            };
            if !is_integer_constant(inner_right, target_type, IntegerValue::Unsigned(5)) {
                return None;
            }
            let cast = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(inner_left)
            })?;
            let OperationKind::IntegerExactCast {
                operand,
                obligation: cast_obligation,
            } = cast.kind
            else {
                return None;
            };
            (operand == entry.parameters[0].id).then_some([
                cast_obligation,
                inner_obligation,
                outer_obligation,
            ])
        })
        .expect("cancellation retains the cast and both arithmetic-prefix obligations");
    for obligations in [
        finite_cast_then_offset_obligations.as_slice(),
        cancelling_cast_then_offset_obligations.as_slice(),
    ] {
        for (index, obligation) in obligations.iter().enumerate() {
            for other in &obligations[index + 1..] {
                assert_ne!(obligation, other);
            }
            let operation = operations
                .iter()
                .find(|operation| {
                    matches!(
                        operation.kind,
                        OperationKind::IntegerExactCast {
                            obligation: candidate,
                            ..
                        } | OperationKind::ExactIntegerAdd {
                            obligation: candidate,
                            ..
                        } | OperationKind::ExactIntegerSubtract {
                            obligation: candidate,
                            ..
                        } if candidate == *obligation
                    )
                })
                .expect("finite cast-then-offset obligation retains its exact operation");
            assert_eq!(
                TerminalFuelSchedule::CURRENT.operation_units(&operation.kind),
                1
            );
            assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
                evidence.obligation == *obligation
                    && matches!(
                        evidence.route,
                        proof_admission::EvidenceRoute::CertificateDerived(_)
                    )
            }));
        }
    }
    let find_cast_then_multiply_chain = |outer_factor| {
        operations.iter().find_map(|outer| {
            let OperationKind::ExactIntegerMultiply {
                left,
                right,
                obligation: outer_obligation,
            } = outer.kind
            else {
                return None;
            };
            if !is_integer_constant(right, target_type, IntegerValue::Unsigned(outer_factor)) {
                return None;
            }
            let inner = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(left)
            })?;
            let OperationKind::ExactIntegerMultiply {
                left: inner_left,
                right: inner_right,
                obligation: inner_obligation,
            } = inner.kind
            else {
                return None;
            };
            if !is_u8_two(inner_right) {
                return None;
            }
            let cast = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(inner_left)
            })?;
            let OperationKind::IntegerExactCast {
                operand,
                obligation: cast_obligation,
            } = cast.kind
            else {
                return None;
            };
            (operand == entry.parameters[0].id)
                .then_some(([cast_obligation, inner_obligation, outer_obligation], right))
        })
    };
    let (cast_then_multiply_obligations, cast_then_multiply_outer_factor) =
        find_cast_then_multiply_chain(3)
            .expect("one direct exact cast roots a finite exact-multiply chain");
    let (zero_cast_then_multiply_obligations, _) = find_cast_then_multiply_chain(0)
        .expect("a zero factor retains all prior post-cast multiply-prefix obligations");
    for obligations in [
        cast_then_multiply_obligations.as_slice(),
        zero_cast_then_multiply_obligations.as_slice(),
    ] {
        for (index, obligation) in obligations.iter().enumerate() {
            for other in &obligations[index + 1..] {
                assert_ne!(obligation, other);
            }
            let operation = operations
                .iter()
                .find(|operation| {
                    matches!(
                        operation.kind,
                        OperationKind::IntegerExactCast {
                            obligation: candidate,
                            ..
                        } | OperationKind::ExactIntegerMultiply {
                            obligation: candidate,
                            ..
                        } if candidate == *obligation
                    )
                })
                .expect("post-cast multiply obligation retains its exact operation");
            assert_eq!(
                TerminalFuelSchedule::CURRENT.operation_units(&operation.kind),
                1
            );
            assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
                evidence.obligation == *obligation
                    && matches!(
                        evidence.route,
                        proof_admission::EvidenceRoute::CertificateDerived(_)
                    )
            }));
        }
    }
    let find_cast_then_affine = |signed: bool, zero: bool| {
        operations.iter().find_map(|outer| {
            let (left, right, outer_obligation) = match outer.kind {
                OperationKind::ExactIntegerAdd {
                    left,
                    right,
                    obligation,
                } if signed || zero => (left, right, obligation),
                OperationKind::ExactIntegerSubtract {
                    left,
                    right,
                    obligation,
                } if !signed && !zero => (left, right, obligation),
                _ => return None,
            };
            let expected_type = if signed { i8_type } else { target_type };
            let expected_outer = if signed {
                IntegerValue::Signed(1)
            } else if zero {
                IntegerValue::Unsigned(255)
            } else {
                IntegerValue::Unsigned(1)
            };
            if !is_integer_constant(right, expected_type, expected_outer) {
                return None;
            }
            let middle = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(left)
            })?;
            let OperationKind::ExactIntegerMultiply {
                left: middle_left,
                right: middle_right,
                obligation: middle_obligation,
            } = middle.kind
            else {
                return None;
            };
            let expected_factor = if signed {
                IntegerValue::Signed(2)
            } else if zero {
                IntegerValue::Unsigned(0)
            } else {
                IntegerValue::Unsigned(2)
            };
            if !is_integer_constant(middle_right, expected_type, expected_factor) {
                return None;
            }
            let inner = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(middle_left)
            })?;
            let (inner_left, inner_right, inner_obligation) = match inner.kind {
                OperationKind::ExactIntegerSubtract {
                    left,
                    right,
                    obligation,
                } if signed => (left, right, obligation),
                OperationKind::ExactIntegerAdd {
                    left,
                    right,
                    obligation,
                } if !signed => (left, right, obligation),
                _ => return None,
            };
            let expected_inner = if signed {
                IntegerValue::Signed(3)
            } else {
                IntegerValue::Unsigned(3)
            };
            if !is_integer_constant(inner_right, expected_type, expected_inner) {
                return None;
            }
            let cast = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(inner_left)
            })?;
            let OperationKind::IntegerExactCast {
                operand,
                obligation: cast_obligation,
            } = cast.kind
            else {
                return None;
            };
            let parameter = if signed {
                entry.parameters[4].id
            } else {
                entry.parameters[0].id
            };
            (operand == parameter).then_some((
                [
                    cast_obligation,
                    inner_obligation,
                    middle_obligation,
                    outer_obligation,
                ],
                middle_right,
            ))
        })
    };
    let (cast_then_affine_obligations, cast_then_affine_factor) =
        find_cast_then_affine(false, false)
            .expect("one direct partial cast roots a mixed exact-affine chain");
    let (zero_cast_then_affine_obligations, _) = find_cast_then_affine(false, true)
        .expect("zero collapse retains the cast and every affine-prefix proof");
    let (signed_cast_then_affine_obligations, _) = find_cast_then_affine(true, false)
        .expect("one signed partial cast roots a mixed exact-affine chain");
    for obligations in [
        cast_then_affine_obligations.as_slice(),
        zero_cast_then_affine_obligations.as_slice(),
        signed_cast_then_affine_obligations.as_slice(),
    ] {
        for (index, obligation) in obligations.iter().enumerate() {
            for other in &obligations[index + 1..] {
                assert_ne!(obligation, other);
            }
            let operation = operations
                .iter()
                .find(|operation| {
                    matches!(
                        operation.kind,
                        OperationKind::IntegerExactCast {
                            obligation: candidate,
                            ..
                        } | OperationKind::ExactIntegerAdd {
                            obligation: candidate,
                            ..
                        } | OperationKind::ExactIntegerSubtract {
                            obligation: candidate,
                            ..
                        } | OperationKind::ExactIntegerMultiply {
                            obligation: candidate,
                            ..
                        } if candidate == *obligation
                    )
                })
                .expect("post-cast affine obligation retains its exact operation");
            assert_eq!(
                TerminalFuelSchedule::CURRENT.operation_units(&operation.kind),
                1
            );
            assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
                evidence.obligation == *obligation
                    && matches!(evidence.route, EvidenceRoute::CertificateDerived(_))
            }));
        }
    }
    let find_multiply_chain_then_cast = |outer_factor| {
        operations.iter().find_map(|cast| {
            let OperationKind::IntegerExactCast {
                operand,
                obligation: cast_obligation,
            } = cast.kind
            else {
                return None;
            };
            let outer = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(operand)
            })?;
            let OperationKind::ExactIntegerMultiply {
                left,
                right,
                obligation: outer_obligation,
            } = outer.kind
            else {
                return None;
            };
            if !is_integer_constant(right, target_type, IntegerValue::Unsigned(outer_factor)) {
                return None;
            }
            let inner = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(left)
            })?;
            let OperationKind::ExactIntegerMultiply {
                left: inner_left,
                right: inner_right,
                obligation: inner_obligation,
            } = inner.kind
            else {
                return None;
            };
            (inner_left == entry.parameters[1].id && is_u8_two(inner_right))
                .then_some(([inner_obligation, outer_obligation, cast_obligation], right))
        })
    };
    let (multiply_chain_then_cast_obligations, multiply_chain_then_cast_outer_factor) =
        find_multiply_chain_then_cast(3)
            .expect("a finite exact-multiply chain feeds one partial exact cast");
    let (zero_multiply_chain_then_cast_obligations, _) = find_multiply_chain_then_cast(0)
        .expect("a zero cumulative product retains both prefixes and the following cast");
    for obligations in [
        multiply_chain_then_cast_obligations.as_slice(),
        zero_multiply_chain_then_cast_obligations.as_slice(),
    ] {
        for (index, obligation) in obligations.iter().enumerate() {
            for other in &obligations[index + 1..] {
                assert_ne!(obligation, other);
            }
            let operation = operations
                .iter()
                .find(|operation| {
                    matches!(
                        operation.kind,
                        OperationKind::IntegerExactCast {
                            obligation: candidate,
                            ..
                        } | OperationKind::ExactIntegerMultiply {
                            obligation: candidate,
                            ..
                        } if candidate == *obligation
                    )
                })
                .expect("pre-cast multiply obligation retains its exact operation");
            assert_eq!(
                TerminalFuelSchedule::CURRENT.operation_units(&operation.kind),
                1
            );
            assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
                evidence.obligation == *obligation
                    && matches!(
                        evidence.route,
                        proof_admission::EvidenceRoute::CertificateDerived(_)
                    )
            }));
        }
    }
    let (nested_divide_remainder_obligations, middle_divisor) = operations
        .iter()
        .find_map(|outer| {
            let OperationKind::ExactIntegerDivide {
                left,
                right,
                obligation: outer_obligation,
            } = outer.kind
            else {
                return None;
            };
            if !is_u8_two(right) {
                return None;
            }
            let middle = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(left)
            })?;
            let OperationKind::ExactIntegerRemainder {
                left: middle_left,
                right: middle_right,
                obligation: middle_obligation,
            } = middle.kind
            else {
                return None;
            };
            if !is_u8_three(middle_right) {
                return None;
            }
            let inner = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(middle_left)
            })?;
            let OperationKind::ExactIntegerDivide {
                left: inner_left,
                right: inner_right,
                obligation: inner_obligation,
            } = inner.kind
            else {
                return None;
            };
            (inner_left == entry.parameters[1].id && is_u8_two(inner_right)).then_some((
                [inner_obligation, middle_obligation, outer_obligation],
                middle_right,
            ))
        })
        .expect("a finite mixed exact-divide/remainder chain is retained");
    assert_ne!(
        nested_divide_remainder_obligations[0],
        nested_divide_remainder_obligations[1]
    );
    assert_ne!(
        nested_divide_remainder_obligations[1],
        nested_divide_remainder_obligations[2]
    );
    assert_ne!(
        nested_divide_remainder_obligations[0],
        nested_divide_remainder_obligations[2]
    );
    for obligation in nested_divide_remainder_obligations {
        assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
            evidence.obligation == obligation
                && matches!(
                    evidence.route,
                    proof_admission::EvidenceRoute::CertificateDerived(_)
                )
        }));
    }
    let (cast_then_divide_remainder_obligations, cast_then_divide_remainder_middle_divisor) =
        operations
            .iter()
            .find_map(|outer| {
                let OperationKind::ExactIntegerDivide {
                    left,
                    right,
                    obligation: outer_obligation,
                } = outer.kind
                else {
                    return None;
                };
                if !is_u8_two(right) {
                    return None;
                }
                let middle = operations.iter().find(|candidate| {
                    candidate.result.scalar_ref().map(|result| result.id) == Some(left)
                })?;
                let OperationKind::ExactIntegerRemainder {
                    left: middle_left,
                    right: middle_right,
                    obligation: middle_obligation,
                } = middle.kind
                else {
                    return None;
                };
                if !is_u8_three(middle_right) {
                    return None;
                }
                let inner = operations.iter().find(|candidate| {
                    candidate.result.scalar_ref().map(|result| result.id) == Some(middle_left)
                })?;
                let OperationKind::ExactIntegerDivide {
                    left: inner_left,
                    right: inner_right,
                    obligation: inner_obligation,
                } = inner.kind
                else {
                    return None;
                };
                if !is_u8_two(inner_right) {
                    return None;
                }
                let cast = operations.iter().find(|candidate| {
                    candidate.result.scalar_ref().map(|result| result.id) == Some(inner_left)
                })?;
                let OperationKind::IntegerExactCast {
                    operand,
                    obligation: cast_obligation,
                } = cast.kind
                else {
                    return None;
                };
                (operand == entry.parameters[0].id).then_some((
                    [
                        cast_obligation,
                        inner_obligation,
                        middle_obligation,
                        outer_obligation,
                    ],
                    middle_right,
                ))
            })
            .expect("one direct exact cast roots a finite divide/remainder chain");
    let find_two_link_cast_then_divide_remainder = |parameter, signed| {
        operations.iter().find_map(|outer| {
            let OperationKind::ExactIntegerRemainder {
                left,
                right,
                obligation: outer_obligation,
            } = outer.kind
            else {
                return None;
            };
            let outer_matches = if signed {
                is_integer_constant(right, i8_type, IntegerValue::Signed(-3))
            } else {
                is_u8_three(right)
            };
            if !outer_matches {
                return None;
            }
            let inner = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(left)
            })?;
            let OperationKind::ExactIntegerDivide {
                left: inner_left,
                right: inner_right,
                obligation: inner_obligation,
            } = inner.kind
            else {
                return None;
            };
            let inner_matches = if signed {
                is_integer_constant(inner_right, i8_type, IntegerValue::Signed(2))
            } else {
                is_u8_two(inner_right)
            };
            if !inner_matches {
                return None;
            }
            let cast = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(inner_left)
            })?;
            let OperationKind::IntegerExactCast {
                operand,
                obligation: cast_obligation,
            } = cast.kind
            else {
                return None;
            };
            (operand == parameter).then_some([cast_obligation, inner_obligation, outer_obligation])
        })
    };
    let signed_cast_then_divide_remainder_obligations =
        find_two_link_cast_then_divide_remainder(entry.parameters[4].id, true)
            .expect("one signed exact cast roots a finite divide/remainder chain");
    let cross_cast_then_divide_remainder_obligations =
        find_two_link_cast_then_divide_remainder(entry.parameters[5].id, false)
            .expect("one cross-sign exact cast roots a finite divide/remainder chain");
    for obligations in [
        cast_then_divide_remainder_obligations.as_slice(),
        signed_cast_then_divide_remainder_obligations.as_slice(),
        cross_cast_then_divide_remainder_obligations.as_slice(),
    ] {
        for (index, obligation) in obligations.iter().enumerate() {
            for other in &obligations[index + 1..] {
                assert_ne!(obligation, other);
            }
            let operation = operations
                .iter()
                .find(|operation| {
                    matches!(
                        operation.kind,
                        OperationKind::IntegerExactCast {
                            obligation: candidate,
                            ..
                        } | OperationKind::ExactIntegerDivide {
                            obligation: candidate,
                            ..
                        } | OperationKind::ExactIntegerRemainder {
                            obligation: candidate,
                            ..
                        } if candidate == *obligation
                    )
                })
                .expect("post-cast divide/remainder obligation retains its exact operation");
            assert_eq!(
                TerminalFuelSchedule::CURRENT.operation_units(&operation.kind),
                1
            );
            assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
                evidence.obligation == *obligation
                    && matches!(evidence.route, EvidenceRoute::CertificateDerived(_))
            }));
        }
    }
    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let find_direct_runtime_divisor_chain =
        |root, runtime_divisor, integer_type: IntegerType, outer_value: IntegerValue| {
            operations.iter().find_map(|outer| {
                let OperationKind::ExactIntegerRemainder {
                    left,
                    right,
                    obligation: outer_obligation,
                } = outer.kind
                else {
                    return None;
                };
                if !is_integer_constant(right, integer_type, outer_value) {
                    return None;
                }
                let inner = operations.iter().find(|candidate| {
                    candidate.result.scalar_ref().map(|result| result.id) == Some(left)
                })?;
                let OperationKind::ExactIntegerDivide {
                    left: inner_left,
                    right: inner_right,
                    obligation: inner_obligation,
                } = inner.kind
                else {
                    return None;
                };
                (inner_left == root && inner_right == runtime_divisor)
                    .then_some([inner_obligation, outer_obligation])
            })
        };
    let direct_unsigned_runtime_divisor_obligations = find_direct_runtime_divisor_chain(
        entry.parameters[1].id,
        entry.parameters[2].id,
        u8_type,
        IntegerValue::Unsigned(2),
    )
    .expect("one direct unsigned runtime divisor roots a finite chain");
    let direct_signed_positive_runtime_divisor_obligations = find_direct_runtime_divisor_chain(
        entry.parameters[5].id,
        entry.parameters[6].id,
        i8_type,
        IntegerValue::Signed(-3),
    )
    .expect("one direct signed-positive runtime divisor roots a finite chain");
    let direct_signed_negative_runtime_divisor_obligations = find_direct_runtime_divisor_chain(
        entry.parameters[5].id,
        entry.parameters[7].id,
        i8_type,
        IntegerValue::Signed(3),
    )
    .expect("one direct signed-negative runtime divisor roots a finite chain");
    let find_post_cast_runtime_divisor_chain =
        |root, runtime_divisor, integer_type: IntegerType, outer_value: IntegerValue| {
            operations.iter().find_map(|outer| {
                let OperationKind::ExactIntegerRemainder {
                    left,
                    right,
                    obligation: outer_obligation,
                } = outer.kind
                else {
                    return None;
                };
                if !is_integer_constant(right, integer_type, outer_value) {
                    return None;
                }
                let inner = operations.iter().find(|candidate| {
                    candidate.result.scalar_ref().map(|result| result.id) == Some(left)
                })?;
                let OperationKind::ExactIntegerDivide {
                    left: inner_left,
                    right: inner_right,
                    obligation: inner_obligation,
                } = inner.kind
                else {
                    return None;
                };
                if inner_right != runtime_divisor {
                    return None;
                }
                let cast = operations.iter().find(|candidate| {
                    candidate.result.scalar_ref().map(|result| result.id) == Some(inner_left)
                })?;
                let OperationKind::IntegerExactCast {
                    operand,
                    obligation: cast_obligation,
                } = cast.kind
                else {
                    return None;
                };
                (operand == root).then_some([cast_obligation, inner_obligation, outer_obligation])
            })
        };
    let post_cast_unsigned_runtime_divisor_obligations = find_post_cast_runtime_divisor_chain(
        entry.parameters[0].id,
        entry.parameters[2].id,
        u8_type,
        IntegerValue::Unsigned(2),
    )
    .expect("one partial cast roots an unsigned runtime-divisor chain");
    let post_cast_signed_positive_runtime_divisor_obligations =
        find_post_cast_runtime_divisor_chain(
            entry.parameters[4].id,
            entry.parameters[6].id,
            i8_type,
            IntegerValue::Signed(-3),
        )
        .expect("one partial cast roots a signed-positive runtime-divisor chain");
    let post_cast_signed_negative_runtime_divisor_obligations =
        find_post_cast_runtime_divisor_chain(
            entry.parameters[4].id,
            entry.parameters[7].id,
            i8_type,
            IntegerValue::Signed(3),
        )
        .expect("one partial cast roots a signed-negative runtime-divisor chain");
    for obligations in [
        direct_unsigned_runtime_divisor_obligations.as_slice(),
        direct_signed_positive_runtime_divisor_obligations.as_slice(),
        direct_signed_negative_runtime_divisor_obligations.as_slice(),
        post_cast_unsigned_runtime_divisor_obligations.as_slice(),
        post_cast_signed_positive_runtime_divisor_obligations.as_slice(),
        post_cast_signed_negative_runtime_divisor_obligations.as_slice(),
    ] {
        for (index, obligation) in obligations.iter().enumerate() {
            for other in &obligations[index + 1..] {
                assert_ne!(obligation, other);
            }
            let operation = operations
                .iter()
                .find(|operation| {
                    matches!(
                        operation.kind,
                        OperationKind::IntegerExactCast {
                            obligation: candidate,
                            ..
                        } | OperationKind::ExactIntegerDivide {
                            obligation: candidate,
                            ..
                        } | OperationKind::ExactIntegerRemainder {
                            obligation: candidate,
                            ..
                        } if candidate == *obligation
                    )
                })
                .expect("runtime-divisor chain obligation retains its exact operation");
            assert_eq!(
                TerminalFuelSchedule::CURRENT.operation_units(&operation.kind),
                1
            );
            assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
                evidence.obligation == *obligation
                    && matches!(evidence.route, EvidenceRoute::CertificateDerived(_))
            }));
        }
    }
    let u16_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16");
    let i64_type = IntegerType::new(IntegerSign::Signed, 64).expect("i64");
    let wide_parameter = entry.parameters[17].id;
    let find_cast_after_divide_remainder = |parameter, integer_type, divisor, remainder: bool| {
        operations.iter().find_map(|cast| {
            let OperationKind::IntegerExactCast {
                operand,
                obligation: cast_obligation,
            } = cast.kind
            else {
                return None;
            };
            let arithmetic = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(operand)
            })?;
            let (left, right, arithmetic_obligation) = match arithmetic.kind {
                OperationKind::ExactIntegerDivide {
                    left,
                    right,
                    obligation,
                } if !remainder => (left, right, obligation),
                OperationKind::ExactIntegerRemainder {
                    left,
                    right,
                    obligation,
                } if remainder => (left, right, obligation),
                _ => return None,
            };
            (left == parameter && is_integer_constant(right, integer_type, divisor))
                .then_some([arithmetic_obligation, cast_obligation])
        })
    };
    let divide_chain_cast_obligations = find_cast_after_divide_remainder(
        wide_parameter,
        u16_type,
        IntegerValue::Unsigned(256),
        false,
    )
    .expect("one carrier-total divide chain feeds an exact cast");
    let (mixed_divide_remainder_cast_obligations, mixed_cast_divisor) = operations
        .iter()
        .find_map(|cast| {
            let OperationKind::IntegerExactCast {
                operand,
                obligation: cast_obligation,
            } = cast.kind
            else {
                return None;
            };
            let remainder = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(operand)
            })?;
            let OperationKind::ExactIntegerRemainder {
                left,
                right,
                obligation: remainder_obligation,
            } = remainder.kind
            else {
                return None;
            };
            if !is_integer_constant(right, u16_type, IntegerValue::Unsigned(3)) {
                return None;
            }
            let divide = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(left)
            })?;
            let OperationKind::ExactIntegerDivide {
                left: divide_left,
                right: divide_right,
                obligation: divide_obligation,
            } = divide.kind
            else {
                return None;
            };
            (divide_left == wide_parameter
                && is_integer_constant(divide_right, u16_type, IntegerValue::Unsigned(2)))
            .then_some((
                [divide_obligation, remainder_obligation, cast_obligation],
                right,
            ))
        })
        .expect("one carrier-total mixed divide/remainder chain feeds an exact cast");
    let signed_remainder_cast_obligations = find_cast_after_divide_remainder(
        entry.parameters[4].id,
        i64_type,
        IntegerValue::Signed(-3),
        true,
    )
    .expect("one signed carrier-total remainder feeds an exact cast");
    let cross_remainder_cast_obligations =
        find_cast_after_divide_remainder(wide_parameter, u16_type, IntegerValue::Unsigned(3), true)
            .expect("one cross-sign carrier-total remainder feeds an exact cast");
    for obligations in [
        divide_chain_cast_obligations.as_slice(),
        mixed_divide_remainder_cast_obligations.as_slice(),
        signed_remainder_cast_obligations.as_slice(),
        cross_remainder_cast_obligations.as_slice(),
    ] {
        for (index, obligation) in obligations.iter().enumerate() {
            for other in &obligations[index + 1..] {
                assert_ne!(obligation, other);
            }
            let operation = operations
                .iter()
                .find(|operation| {
                    matches!(
                        operation.kind,
                        OperationKind::IntegerExactCast {
                            obligation: candidate,
                            ..
                        } | OperationKind::ExactIntegerDivide {
                            obligation: candidate,
                            ..
                        } | OperationKind::ExactIntegerRemainder {
                            obligation: candidate,
                            ..
                        } if candidate == *obligation
                    )
                })
                .expect("pre-cast divide/remainder obligation retains its exact operation");
            assert_eq!(
                TerminalFuelSchedule::CURRENT.operation_units(&operation.kind),
                1
            );
            assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
                evidence.obligation == *obligation
                    && matches!(evidence.route, EvidenceRoute::CertificateDerived(_))
            }));
        }
    }
    let i32_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
    let (nested_shift_right_obligations, middle_shift_count) = operations
        .iter()
        .find_map(|outer| {
            let OperationKind::ExactIntegerShiftRight {
                value,
                count,
                obligation: outer_obligation,
            } = outer.kind
            else {
                return None;
            };
            if !is_integer_constant(count, i32_type, IntegerValue::Signed(0)) {
                return None;
            }
            let middle = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(value)
            })?;
            let OperationKind::ExactIntegerShiftRight {
                value: middle_value,
                count: middle_count,
                obligation: middle_obligation,
            } = middle.kind
            else {
                return None;
            };
            if !is_integer_constant(middle_count, u16_type, IntegerValue::Unsigned(2)) {
                return None;
            }
            let inner = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(middle_value)
            })?;
            let OperationKind::ExactIntegerShiftRight {
                value: inner_value,
                count: inner_count,
                obligation: inner_obligation,
            } = inner.kind
            else {
                return None;
            };
            (inner_value == entry.parameters[1].id
                && is_integer_constant(inner_count, i8_type, IntegerValue::Signed(1)))
            .then_some((
                [inner_obligation, middle_obligation, outer_obligation],
                middle_count,
            ))
        })
        .expect("a finite exact-shift-right chain with distinct count carriers is retained");
    assert_ne!(
        nested_shift_right_obligations[0],
        nested_shift_right_obligations[1]
    );
    assert_ne!(
        nested_shift_right_obligations[1],
        nested_shift_right_obligations[2]
    );
    assert_ne!(
        nested_shift_right_obligations[0],
        nested_shift_right_obligations[2]
    );
    for obligation in nested_shift_right_obligations {
        assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
            evidence.obligation == obligation
                && matches!(
                    evidence.route,
                    proof_admission::EvidenceRoute::CertificateDerived(_)
                )
        }));
    }
    let (shift_right_then_cast_obligations, shift_right_then_cast_middle_count) = operations
        .iter()
        .find_map(|cast| {
            let OperationKind::IntegerExactCast {
                operand,
                obligation: cast_obligation,
            } = cast.kind
            else {
                return None;
            };
            let outer = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(operand)
            })?;
            let OperationKind::ExactIntegerShiftRight {
                value,
                count,
                obligation: outer_obligation,
            } = outer.kind
            else {
                return None;
            };
            if !is_integer_constant(count, i32_type, IntegerValue::Signed(0)) {
                return None;
            }
            let middle = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(value)
            })?;
            let OperationKind::ExactIntegerShiftRight {
                value: middle_value,
                count: middle_count,
                obligation: middle_obligation,
            } = middle.kind
            else {
                return None;
            };
            if !is_integer_constant(middle_count, u16_type, IntegerValue::Unsigned(2)) {
                return None;
            }
            let inner = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(middle_value)
            })?;
            let OperationKind::ExactIntegerShiftRight {
                value: inner_value,
                count: inner_count,
                obligation: inner_obligation,
            } = inner.kind
            else {
                return None;
            };
            (inner_value == entry.parameters[1].id
                && is_integer_constant(inner_count, i8_type, IntegerValue::Signed(1)))
            .then_some((
                [
                    inner_obligation,
                    middle_obligation,
                    outer_obligation,
                    cast_obligation,
                ],
                middle_count,
            ))
        })
        .expect("one heterogeneous exact-right-shift chain feeds a partial exact cast");
    let zero_shift_right_then_cast_obligations = operations
        .iter()
        .find_map(|cast| {
            let OperationKind::IntegerExactCast {
                operand,
                obligation: cast_obligation,
            } = cast.kind
            else {
                return None;
            };
            let shift = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(operand)
            })?;
            let OperationKind::ExactIntegerShiftRight {
                value,
                count,
                obligation: shift_obligation,
            } = shift.kind
            else {
                return None;
            };
            (value == entry.parameters[1].id
                && is_integer_constant(count, i8_type, IntegerValue::Signed(0)))
            .then_some([shift_obligation, cast_obligation])
        })
        .expect("one zero-count exact-right-shift retains an independent following cast");
    for obligations in [
        shift_right_then_cast_obligations.as_slice(),
        zero_shift_right_then_cast_obligations.as_slice(),
    ] {
        for (index, obligation) in obligations.iter().enumerate() {
            for other in &obligations[index + 1..] {
                assert_ne!(obligation, other);
            }
            let operation = operations
                .iter()
                .find(|operation| {
                    matches!(operation.kind,
                        OperationKind::IntegerExactCast { obligation: candidate, .. }
                        | OperationKind::ExactIntegerShiftRight { obligation: candidate, .. }
                        if candidate == *obligation)
                })
                .expect("pre-cast right-shift obligation operation");
            assert_eq!(
                TerminalFuelSchedule::CURRENT.operation_units(&operation.kind),
                1
            );
            assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
                evidence.obligation == *obligation
                    && matches!(evidence.route, EvidenceRoute::CertificateDerived(_))
            }));
        }
    }
    let (cast_then_shift_right_obligations, cast_then_shift_right_middle_count) = operations
        .iter()
        .find_map(|outer| {
            let OperationKind::ExactIntegerShiftRight {
                value,
                count,
                obligation: outer_obligation,
            } = outer.kind
            else {
                return None;
            };
            if !is_integer_constant(count, i32_type, IntegerValue::Signed(0)) {
                return None;
            }
            let middle = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(value)
            })?;
            let OperationKind::ExactIntegerShiftRight {
                value: middle_value,
                count: middle_count,
                obligation: middle_obligation,
            } = middle.kind
            else {
                return None;
            };
            if !is_integer_constant(middle_count, u16_type, IntegerValue::Unsigned(2)) {
                return None;
            }
            let inner = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(middle_value)
            })?;
            let OperationKind::ExactIntegerShiftRight {
                value: inner_value,
                count: inner_count,
                obligation: inner_obligation,
            } = inner.kind
            else {
                return None;
            };
            if !is_integer_constant(inner_count, i8_type, IntegerValue::Signed(1)) {
                return None;
            }
            let cast = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(inner_value)
            })?;
            let OperationKind::IntegerExactCast {
                operand,
                obligation: cast_obligation,
            } = cast.kind
            else {
                return None;
            };
            (operand == entry.parameters[0].id).then_some((
                [
                    cast_obligation,
                    inner_obligation,
                    middle_obligation,
                    outer_obligation,
                ],
                middle_count,
            ))
        })
        .expect("one direct exact cast roots a heterogeneous finite exact-right-shift chain");
    let find_two_link_cast_then_shift_right =
        |parameter, inner_type, inner_value, outer_type, outer_value| {
            operations.iter().find_map(|outer| {
                let OperationKind::ExactIntegerShiftRight {
                    value,
                    count,
                    obligation: outer_obligation,
                } = outer.kind
                else {
                    return None;
                };
                if !is_integer_constant(count, outer_type, outer_value) {
                    return None;
                }
                let inner = operations.iter().find(|candidate| {
                    candidate.result.scalar_ref().map(|result| result.id) == Some(value)
                })?;
                let OperationKind::ExactIntegerShiftRight {
                    value: inner_value_id,
                    count: inner_count,
                    obligation: inner_obligation,
                } = inner.kind
                else {
                    return None;
                };
                if !is_integer_constant(inner_count, inner_type, inner_value) {
                    return None;
                }
                let cast = operations.iter().find(|candidate| {
                    candidate.result.scalar_ref().map(|result| result.id) == Some(inner_value_id)
                })?;
                let OperationKind::IntegerExactCast {
                    operand,
                    obligation: cast_obligation,
                } = cast.kind
                else {
                    return None;
                };
                (operand == parameter).then_some([
                    cast_obligation,
                    inner_obligation,
                    outer_obligation,
                ])
            })
        };
    let signed_cast_then_shift_right_obligations = find_two_link_cast_then_shift_right(
        entry.parameters[4].id,
        u16_type,
        IntegerValue::Unsigned(1),
        i32_type,
        IntegerValue::Signed(2),
    )
    .expect("one signed direct exact cast roots a heterogeneous right-shift chain");
    let cross_cast_then_shift_right_obligations = find_two_link_cast_then_shift_right(
        entry.parameters[5].id,
        i8_type,
        IntegerValue::Signed(1),
        u16_type,
        IntegerValue::Unsigned(2),
    )
    .expect("one cross-sign direct exact cast roots a heterogeneous right-shift chain");
    for obligations in [
        cast_then_shift_right_obligations.as_slice(),
        signed_cast_then_shift_right_obligations.as_slice(),
        cross_cast_then_shift_right_obligations.as_slice(),
    ] {
        for (index, obligation) in obligations.iter().enumerate() {
            for other in &obligations[index + 1..] {
                assert_ne!(obligation, other);
            }
            let operation = operations
                .iter()
                .find(|operation| {
                    matches!(
                        operation.kind,
                        OperationKind::IntegerExactCast {
                            obligation: candidate,
                            ..
                        } | OperationKind::ExactIntegerShiftRight {
                            obligation: candidate,
                            ..
                        } if candidate == *obligation
                    )
                })
                .expect("post-cast right-shift obligation retains its exact operation");
            assert_eq!(
                TerminalFuelSchedule::CURRENT.operation_units(&operation.kind),
                1
            );
            assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
                evidence.obligation == *obligation
                    && matches!(evidence.route, EvidenceRoute::CertificateDerived(_))
            }));
        }
    }
    let (nested_shift_left_obligations, middle_shift_left_count) = operations
        .iter()
        .find_map(|outer| {
            let OperationKind::ExactIntegerShiftLeft {
                value,
                count,
                obligation: outer_obligation,
            } = outer.kind
            else {
                return None;
            };
            if !is_integer_constant(count, i32_type, IntegerValue::Signed(0)) {
                return None;
            }
            let middle = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(value)
            })?;
            let OperationKind::ExactIntegerShiftLeft {
                value: middle_value,
                count: middle_count,
                obligation: middle_obligation,
            } = middle.kind
            else {
                return None;
            };
            if !is_integer_constant(middle_count, u16_type, IntegerValue::Unsigned(2)) {
                return None;
            }
            let inner = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(middle_value)
            })?;
            let OperationKind::ExactIntegerShiftLeft {
                value: inner_value,
                count: inner_count,
                obligation: inner_obligation,
            } = inner.kind
            else {
                return None;
            };
            (inner_value == entry.parameters[1].id
                && is_integer_constant(inner_count, i8_type, IntegerValue::Signed(1)))
            .then_some((
                [inner_obligation, middle_obligation, outer_obligation],
                middle_count,
            ))
        })
        .expect("a finite exact-shift-left chain with distinct count carriers is retained");
    assert_ne!(
        nested_shift_left_obligations[0],
        nested_shift_left_obligations[1]
    );
    assert_ne!(
        nested_shift_left_obligations[1],
        nested_shift_left_obligations[2]
    );
    assert_ne!(
        nested_shift_left_obligations[0],
        nested_shift_left_obligations[2]
    );
    for obligation in nested_shift_left_obligations {
        assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
            evidence.obligation == obligation
                && matches!(
                    evidence.route,
                    proof_admission::EvidenceRoute::CertificateDerived(_)
                )
        }));
    }
    let (cast_then_shift_left_obligations, cast_then_shift_left_middle_count) = operations
        .iter()
        .find_map(|outer| {
            let OperationKind::ExactIntegerShiftLeft {
                value,
                count,
                obligation: outer_obligation,
            } = outer.kind
            else {
                return None;
            };
            if !is_integer_constant(count, i32_type, IntegerValue::Signed(0)) {
                return None;
            }
            let middle = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(value)
            })?;
            let OperationKind::ExactIntegerShiftLeft {
                value: middle_value,
                count: middle_count,
                obligation: middle_obligation,
            } = middle.kind
            else {
                return None;
            };
            if !is_integer_constant(middle_count, u16_type, IntegerValue::Unsigned(2)) {
                return None;
            }
            let inner = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(middle_value)
            })?;
            let OperationKind::ExactIntegerShiftLeft {
                value: inner_value,
                count: inner_count,
                obligation: inner_obligation,
            } = inner.kind
            else {
                return None;
            };
            if !is_integer_constant(inner_count, i8_type, IntegerValue::Signed(1)) {
                return None;
            }
            let cast = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(inner_value)
            })?;
            let OperationKind::IntegerExactCast {
                operand,
                obligation: cast_obligation,
            } = cast.kind
            else {
                return None;
            };
            (operand == entry.parameters[0].id).then_some((
                [
                    cast_obligation,
                    inner_obligation,
                    middle_obligation,
                    outer_obligation,
                ],
                middle_count,
            ))
        })
        .expect("one direct exact cast roots a heterogeneous finite exact-left-shift chain");
    for (index, obligation) in cast_then_shift_left_obligations.iter().enumerate() {
        for other in &cast_then_shift_left_obligations[index + 1..] {
            assert_ne!(obligation, other);
        }
        let operation = operations
            .iter()
            .find(|operation| {
                matches!(
                    operation.kind,
                    OperationKind::IntegerExactCast {
                        obligation: candidate,
                        ..
                    } | OperationKind::ExactIntegerShiftLeft {
                        obligation: candidate,
                        ..
                    } if candidate == *obligation
                )
            })
            .expect("post-cast shift-left obligation retains its exact operation");
        assert_eq!(
            TerminalFuelSchedule::CURRENT.operation_units(&operation.kind),
            1
        );
        assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
            evidence.obligation == *obligation
                && matches!(
                    evidence.route,
                    proof_admission::EvidenceRoute::CertificateDerived(_)
                )
        }));
    }
    let (shift_left_then_cast_obligations, shift_left_then_cast_middle_count) = operations
        .iter()
        .find_map(|cast| {
            let OperationKind::IntegerExactCast {
                operand,
                obligation: cast_obligation,
            } = cast.kind
            else {
                return None;
            };
            let outer = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(operand)
            })?;
            let OperationKind::ExactIntegerShiftLeft {
                value,
                count,
                obligation: outer_obligation,
            } = outer.kind
            else {
                return None;
            };
            if !is_integer_constant(count, i32_type, IntegerValue::Signed(0)) {
                return None;
            }
            let middle = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(value)
            })?;
            let OperationKind::ExactIntegerShiftLeft {
                value: middle_value,
                count: middle_count,
                obligation: middle_obligation,
            } = middle.kind
            else {
                return None;
            };
            if !is_integer_constant(middle_count, u16_type, IntegerValue::Unsigned(2)) {
                return None;
            }
            let inner = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(middle_value)
            })?;
            let OperationKind::ExactIntegerShiftLeft {
                value: inner_value,
                count: inner_count,
                obligation: inner_obligation,
            } = inner.kind
            else {
                return None;
            };
            (inner_value == entry.parameters[1].id
                && is_integer_constant(inner_count, i8_type, IntegerValue::Signed(1)))
            .then_some((
                [
                    inner_obligation,
                    middle_obligation,
                    outer_obligation,
                    cast_obligation,
                ],
                middle_count,
            ))
        })
        .expect("one heterogeneous finite exact-left-shift chain feeds a partial exact cast");
    let zero_shift_then_cast_obligations = operations
        .iter()
        .find_map(|cast| {
            let OperationKind::IntegerExactCast {
                operand,
                obligation: cast_obligation,
            } = cast.kind
            else {
                return None;
            };
            let shift = operations.iter().find(|candidate| {
                candidate.result.scalar_ref().map(|result| result.id) == Some(operand)
            })?;
            let OperationKind::ExactIntegerShiftLeft {
                value,
                count,
                obligation: shift_obligation,
            } = shift.kind
            else {
                return None;
            };
            (value == entry.parameters[1].id
                && is_integer_constant(count, i8_type, IntegerValue::Signed(0)))
            .then_some([shift_obligation, cast_obligation])
        })
        .expect("one zero-count exact-left-shift retains an independent following cast");
    for obligations in [
        shift_left_then_cast_obligations.as_slice(),
        zero_shift_then_cast_obligations.as_slice(),
    ] {
        for (index, obligation) in obligations.iter().enumerate() {
            for other in &obligations[index + 1..] {
                assert_ne!(obligation, other);
            }
            let operation = operations
                .iter()
                .find(|operation| {
                    matches!(
                        operation.kind,
                        OperationKind::IntegerExactCast {
                            obligation: candidate,
                            ..
                        } | OperationKind::ExactIntegerShiftLeft {
                            obligation: candidate,
                            ..
                        } if candidate == *obligation
                    )
                })
                .expect("pre-cast shift-left obligation retains its exact operation");
            assert_eq!(
                TerminalFuelSchedule::CURRENT.operation_units(&operation.kind),
                1
            );
            assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
                evidence.obligation == *obligation
                    && matches!(
                        evidence.route,
                        proof_admission::EvidenceRoute::CertificateDerived(_)
                    )
            }));
        }
    }
    assert!(entry.blocks.iter().any(|block| {
        block
            .operations
            .iter()
            .any(|operation| matches!(operation.kind, OperationKind::IntegerLessOrEqual { .. }))
    }));
    assert!(entry.blocks.iter().any(|block| {
        block
            .operations
            .iter()
            .any(|operation| matches!(operation.kind, OperationKind::IntegerEqual { .. }))
    }));
    assert_eq!(
        entry
            .blocks
            .iter()
            .filter(|block| matches!(block.terminator, Terminator::Return { .. }))
            .count(),
        1
    );
    let (convergence, control) = entry
        .blocks
        .split_last()
        .expect("shared integer convergence has one cleanup tail");
    assert!(control.iter().any(|block| {
        matches!(
            block.terminator,
            Terminator::Jump { target, .. } if target == convergence.id
        )
    }));
    let finite_roundtrip_cast_obligation = entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| {
            let OperationKind::IntegerExactCast {
                operand,
                obligation,
            } = operation.kind
            else {
                return None;
            };
            let wide_input = entry
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .find_map(|candidate| {
                    (candidate.result.scalar_ref().map(|result| result.id) == Some(operand))
                        .then_some(match candidate.kind {
                            OperationKind::IntegerWiden { operand } => Some(operand),
                            _ => None,
                        })
                        .flatten()
                })?;
            let middle_input = entry
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .find_map(|candidate| {
                    (candidate.result.scalar_ref().map(|result| result.id) == Some(wide_input))
                        .then_some(match candidate.kind {
                            OperationKind::IntegerWiden { operand } => Some(operand),
                            _ => None,
                        })
                        .flatten()
                })?;
            entry
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .any(|candidate| {
                    candidate.result.scalar_ref().map(|result| result.id) == Some(middle_input)
                        && matches!(
                            candidate.kind,
                            OperationKind::IntegerWiden { operand }
                                if operand == entry.parameters[1].id
                        )
                })
                .then_some(obligation)
        })
        .expect("shared convergence retains the complete finite widening-chain round trip");
    assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
        evidence.obligation == finite_roundtrip_cast_obligation
            && matches!(
                evidence.route,
                proof_admission::EvidenceRoute::CertificateDerived(_)
            )
    }));

    let verified = terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("shared integer convergence verifies");
    let fixed = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
        .expect("shared integer convergence has fixed fuel");
    validate_fixed_entry_fuel(&verified, &fixed)
        .expect("shared integer convergence fuel recomputes");
    drop(verified);
    let semantics =
        encode_module(&lowered.semantic_module).expect("shared integer convergence encodes");
    assert_eq!(decode_module(&semantics).unwrap(), lowered.semantic_module);
    let proof = encode_proof_bundle(&lowered.proof_bundle)
        .expect("shared integer convergence proof encodes");
    assert_eq!(
        decode_proof_bundle(&proof).expect("shared integer convergence proof decodes"),
        lowered.proof_bundle,
    );
    let stale_transitive_obligation = transitive_bound_negative_one_obligations[0].0;
    let mut stale_transitive_bound =
        decode_proof_bundle(&proof).expect("decode transitive-bound proof mutation");
    let stale_evidence = stale_transitive_bound
        .evidence
        .iter_mut()
        .find(|evidence| evidence.obligation == stale_transitive_obligation)
        .expect("transitive-bound operation retains exact evidence");
    let EvidenceRoute::CertificateDerived(certificate) = &mut stale_evidence.route else {
        panic!("transitive-bound operation retains a recursive certificate")
    };
    let ProofRule::DisjunctionIntroduction { disjunct, .. } = &mut certificate.proof.rule else {
        panic!("transitive-bound operation retains the exceptional disjunct")
    };
    let ProofRule::ConjunctionIntroduction(conjuncts) = &mut disjunct.rule else {
        panic!("transitive-bound operation retains both exceptional premises")
    };
    let ProofRule::IntegerLessOrEqualTransitivity {
        middle_less_or_equal_right,
        ..
    } = &mut conjuncts[1].rule
    else {
        panic!("transitive-bound operation retains its exact prior-bound citation")
    };
    middle_less_or_equal_right.rule = ProofRule::Assumption { index: usize::MAX };
    assert!(matches!(
        terminal_verifier::verify_module(
            &decode_module(&semantics).expect("decode transitive-bound semantics"),
            &stale_transitive_bound,
            &AdmissionProfile::default(),
        ),
        Err(terminal_verifier::VerificationError::RejectedEvidence {
            obligation,
            ..
        }) if obligation == stale_transitive_obligation
    ));
    let stale_safe_divisor_obligation = runtime_signed_division_obligations[0];
    let mut stale_safe_divisor =
        decode_proof_bundle(&proof).expect("decode safe-divisor proof mutation");
    let stale_evidence = stale_safe_divisor
        .evidence
        .iter_mut()
        .find(|evidence| evidence.obligation == stale_safe_divisor_obligation)
        .expect("safe runtime-divisor operation retains exact evidence");
    let EvidenceRoute::CertificateDerived(certificate) = &mut stale_evidence.route else {
        panic!("safe runtime-divisor operation retains a recursive certificate")
    };
    let ProofRule::DisjunctionIntroduction { disjunct, .. } = &mut certificate.proof.rule else {
        panic!("safe runtime divisor retains its canonical disjunct")
    };
    disjunct.rule = ProofRule::Assumption { index: usize::MAX };
    assert!(matches!(
        terminal_verifier::verify_module(
            &decode_module(&semantics).expect("decode safe-divisor semantics"),
            &stale_safe_divisor,
            &AdmissionProfile::default(),
        ),
        Err(terminal_verifier::VerificationError::RejectedEvidence {
            obligation,
            ..
        }) if obligation == stale_safe_divisor_obligation
    ));
    // The canonical source-to-terminal path and interpreter remain part of
    // every test run. The exhaustive mutation matrix performs 92 independent
    // full verifier replays and is intentionally opt-in; focused verifier
    // rejection tests cover those semantic families in the default suite.
    if std::env::var_os("OMEGA_EXHAUSTIVE_TERMINAL_TAMPER_TESTS").is_some() {
    let mut missing_cast_proof = decode_proof_bundle(&proof).expect("decode shared proof");
    missing_cast_proof
        .evidence
        .retain(|evidence| evidence.obligation != cast_obligation);
    assert!(matches!(
        terminal_verifier::verify_module(
            &decode_module(&semantics).expect("decode shared semantics"),
            &missing_cast_proof,
            &AdmissionProfile::default(),
        ),
        Err(terminal_verifier::VerificationError::MissingEvidence(obligation))
            if obligation == cast_obligation
    ));
    let mut missing_signed_cast_proof = decode_proof_bundle(&proof).expect("decode shared proof");
    missing_signed_cast_proof
        .evidence
        .retain(|evidence| evidence.obligation != signed_cast_obligation);
    assert!(matches!(
        terminal_verifier::verify_module(
            &decode_module(&semantics).expect("decode shared semantics"),
            &missing_signed_cast_proof,
            &AdmissionProfile::default(),
        ),
        Err(terminal_verifier::VerificationError::MissingEvidence(obligation))
            if obligation == signed_cast_obligation
    ));
    for (signed_add_obligation, _) in &signed_add_sites {
        let mut missing_signed_add_proof =
            decode_proof_bundle(&proof).expect("decode shared proof");
        missing_signed_add_proof
            .evidence
            .retain(|evidence| evidence.obligation != *signed_add_obligation);
        assert!(matches!(
            terminal_verifier::verify_module(
                &decode_module(&semantics).expect("decode shared semantics"),
                &missing_signed_add_proof,
                &AdmissionProfile::default(),
            ),
            Err(terminal_verifier::VerificationError::MissingEvidence(obligation))
                if obligation == *signed_add_obligation
        ));
    }
    for cross_sign_cast_obligation in &cross_sign_cast_obligations {
        let mut missing_cross_sign_cast_proof =
            decode_proof_bundle(&proof).expect("decode shared proof");
        missing_cross_sign_cast_proof
            .evidence
            .retain(|evidence| evidence.obligation != *cross_sign_cast_obligation);
        assert!(matches!(
            terminal_verifier::verify_module(
                &decode_module(&semantics).expect("decode shared semantics"),
                &missing_cross_sign_cast_proof,
                &AdmissionProfile::default(),
            ),
            Err(terminal_verifier::VerificationError::MissingEvidence(obligation))
                if obligation == *cross_sign_cast_obligation
        ));
    }
    let mut missing_roundtrip_cast_proof =
        decode_proof_bundle(&proof).expect("decode shared proof");
    missing_roundtrip_cast_proof
        .evidence
        .retain(|evidence| evidence.obligation != roundtrip_cast_obligation);
    assert!(matches!(
        terminal_verifier::verify_module(
            &decode_module(&semantics).expect("decode shared semantics"),
            &missing_roundtrip_cast_proof,
            &AdmissionProfile::default(),
        ),
        Err(terminal_verifier::VerificationError::MissingEvidence(obligation))
            if obligation == roundtrip_cast_obligation
    ));
    let mut redirected_roundtrip_cast = decode_module(&semantics).expect("decode shared semantics");
    let u16_type = IntegerType::new(IntegerSign::Unsigned, 16).unwrap();
    let constant_256 = redirected_roundtrip_cast
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .find_map(|operation| {
            (operation
                .result
                .scalar_ref()
                .map(|result| result.scalar_type)
                == Some(ScalarType::Integer(u16_type))
                && matches!(
                    operation.kind,
                    OperationKind::IntegerConstant {
                        value: IntegerValue::Unsigned(256)
                    }
                ))
            .then(|| operation.result.scalar_ref().expect("scalar constant").id)
        })
        .expect("an earlier u16 256 comparison constant exists");
    let changed_cast = redirected_roundtrip_cast
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            matches!(
                operation.kind,
                OperationKind::IntegerExactCast { obligation, .. }
                    if obligation == roundtrip_cast_obligation
            )
        })
        .expect("roundtrip exact-cast operation exists");
    let OperationKind::IntegerExactCast { operand, .. } = &mut changed_cast.kind else {
        unreachable!("selected exact-cast operation")
    };
    *operand = constant_256;
    assert!(matches!(
        terminal_verifier::verify_module(
            &redirected_roundtrip_cast,
            &decode_proof_bundle(&proof).expect("decode unchanged shared proof"),
            &AdmissionProfile::default(),
        ),
        Err(terminal_verifier::VerificationError::RejectedEvidence {
            obligation,
            ..
        }) if obligation == roundtrip_cast_obligation
    ));
    let mut missing_finite_roundtrip_cast_proof =
        decode_proof_bundle(&proof).expect("decode shared proof");
    missing_finite_roundtrip_cast_proof
        .evidence
        .retain(|evidence| evidence.obligation != finite_roundtrip_cast_obligation);
    assert!(matches!(
        terminal_verifier::verify_module(
            &decode_module(&semantics).expect("decode shared semantics"),
            &missing_finite_roundtrip_cast_proof,
            &AdmissionProfile::default(),
        ),
        Err(terminal_verifier::VerificationError::MissingEvidence(obligation))
            if obligation == finite_roundtrip_cast_obligation
    ));
    let mut redirected_multistep_widen =
        decode_module(&semantics).expect("decode shared semantics");
    let outer_widen_result = redirected_multistep_widen
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .find_map(|operation| match operation.kind {
            OperationKind::IntegerExactCast {
                operand,
                obligation,
            } if obligation == finite_roundtrip_cast_obligation => Some(operand),
            _ => None,
        })
        .expect("finite-chain exact cast retains its outer widening result");
    let redirected_wide_result = redirected_multistep_widen
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .find_map(|operation| {
            (operation.result.scalar_ref().map(|result| result.id) == Some(outer_widen_result))
                .then_some(match operation.kind {
                    OperationKind::IntegerWiden { operand } => Some(operand),
                    _ => None,
                })
                .flatten()
        })
        .expect("outer widening retains its prior chain value");
    let changed_widen = redirected_multistep_widen
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            operation.result.scalar_ref().map(|result| result.id) == Some(redirected_wide_result)
                && matches!(operation.kind, OperationKind::IntegerWiden { .. })
        })
        .expect("redirected middle widening operation exists");
    let OperationKind::IntegerWiden { operand } = &mut changed_widen.kind else {
        unreachable!("selected middle widening operation")
    };
    *operand = constant_256;
    assert!(matches!(
        terminal_verifier::verify_module(
            &redirected_multistep_widen,
            &decode_proof_bundle(&proof).expect("decode unchanged shared proof"),
            &AdmissionProfile::default(),
        ),
        Err(terminal_verifier::VerificationError::RejectedEvidence {
            obligation,
            ..
        }) if obligation == finite_roundtrip_cast_obligation
    ));
    for (signed_subtract_obligation, _) in &signed_subtract_sites {
        let mut missing_signed_subtract_proof =
            decode_proof_bundle(&proof).expect("decode shared proof");
        missing_signed_subtract_proof
            .evidence
            .retain(|evidence| evidence.obligation != *signed_subtract_obligation);
        assert!(matches!(
            terminal_verifier::verify_module(
                &decode_module(&semantics).expect("decode shared semantics"),
                &missing_signed_subtract_proof,
                &AdmissionProfile::default(),
            ),
            Err(terminal_verifier::VerificationError::MissingEvidence(obligation))
                if obligation == *signed_subtract_obligation
        ));
    }
    for (signed_multiply_obligation, _) in &signed_multiply_sites {
        let mut missing_signed_multiply_proof =
            decode_proof_bundle(&proof).expect("decode shared proof");
        missing_signed_multiply_proof
            .evidence
            .retain(|evidence| evidence.obligation != *signed_multiply_obligation);
        assert!(matches!(
            terminal_verifier::verify_module(
                &decode_module(&semantics).expect("decode shared semantics"),
                &missing_signed_multiply_proof,
                &AdmissionProfile::default(),
            ),
            Err(terminal_verifier::VerificationError::MissingEvidence(obligation))
                if obligation == *signed_multiply_obligation
        ));
    }
    for signed_division_obligation in &signed_division_obligations {
        let mut missing_signed_division_proof =
            decode_proof_bundle(&proof).expect("decode shared proof");
        missing_signed_division_proof
            .evidence
            .retain(|evidence| evidence.obligation != *signed_division_obligation);
        assert!(matches!(
            terminal_verifier::verify_module(
                &decode_module(&semantics).expect("decode shared semantics"),
                &missing_signed_division_proof,
                &AdmissionProfile::default(),
            ),
            Err(terminal_verifier::VerificationError::MissingEvidence(obligation))
                if obligation == *signed_division_obligation
        ));
    }
    let mut missing_runtime_subtract_proof =
        decode_proof_bundle(&proof).expect("decode shared proof");
    missing_runtime_subtract_proof
        .evidence
        .retain(|evidence| evidence.obligation != runtime_exact_subtract_obligation);
    assert!(matches!(
        terminal_verifier::verify_module(
            &decode_module(&semantics).expect("decode shared semantics"),
            &missing_runtime_subtract_proof,
            &AdmissionProfile::default(),
        ),
        Err(terminal_verifier::VerificationError::MissingEvidence(obligation))
            if obligation == runtime_exact_subtract_obligation
    ));
    let mut changed_runtime_subtract_requirement =
        decode_module(&semantics).expect("decode shared semantics");
    let changed_entry = changed_runtime_subtract_requirement.entry;
    let entry_contract = &mut changed_runtime_subtract_requirement
        .machines
        .iter_mut()
        .find(|machine| machine.id == changed_entry)
        .expect("changed shared entry")
        .contract;
    let runtime_subtract_requirement_position = entry_contract
        .requires
        .iter()
        .position(|requirement| requirement == &runtime_subtract_requirement)
        .expect("shared convergence retains the runtime-subtract relation");
    entry_contract.requires[runtime_subtract_requirement_position] = Proposition::LessOrEqual(
        ScalarTerm::value(entry.parameters[1].id, entry.parameters[1].scalar_type),
        ScalarTerm::value(entry.parameters[2].id, entry.parameters[2].scalar_type),
    );
    assert!(matches!(
        terminal_verifier::verify_module(
            &changed_runtime_subtract_requirement,
            &decode_proof_bundle(&proof).expect("decode unchanged shared proof"),
            &AdmissionProfile::default(),
        ),
        Err(terminal_verifier::VerificationError::RejectedEvidence {
            obligation,
            ..
        }) if obligation == runtime_exact_subtract_obligation
    ));
    let mut changed_bound = decode_module(&semantics).expect("decode shared semantics");
    let changed_entry = changed_bound.entry;
    let entry_contract = &mut changed_bound
        .machines
        .iter_mut()
        .find(|machine| machine.id == changed_entry)
        .expect("changed shared entry")
        .contract;
    let input_requirement = entry_contract
        .requires
        .iter()
        .position(|requirement| requirement == &input_upper_requirement)
        .expect("shared convergence retains the exact-cast upper-bound premise");
    entry_contract.requires[input_requirement] = Proposition::LessOrEqual(
        input_term,
        ScalarTerm::integer(
            IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
            IntegerValue::Unsigned(254),
        )
        .unwrap(),
    );
    assert!(matches!(
        terminal_verifier::verify_module(
            &changed_bound,
            &decode_proof_bundle(&proof).expect("decode unchanged shared proof"),
            &AdmissionProfile::default(),
        ),
        Err(terminal_verifier::VerificationError::RejectedEvidence {
            obligation,
            ..
        }) if obligation == cast_obligation
    ));
    let mut missing_exact_add_proof = decode_proof_bundle(&proof).expect("decode shared proof");
    missing_exact_add_proof
        .evidence
        .retain(|evidence| evidence.obligation != exact_add_obligation);
    assert!(matches!(
        terminal_verifier::verify_module(
            &decode_module(&semantics).expect("decode shared semantics"),
            &missing_exact_add_proof,
            &AdmissionProfile::default(),
        ),
        Err(terminal_verifier::VerificationError::MissingEvidence(obligation))
            if obligation == exact_add_obligation
    ));
    for nested_add_obligation in nested_add_obligations {
        let mut missing_nested_add_proof =
            decode_proof_bundle(&proof).expect("decode shared proof");
        missing_nested_add_proof
            .evidence
            .retain(|evidence| evidence.obligation != nested_add_obligation);
        assert!(matches!(
            terminal_verifier::verify_module(
                &decode_module(&semantics).expect("decode shared semantics"),
                &missing_nested_add_proof,
                &AdmissionProfile::default(),
            ),
            Err(terminal_verifier::VerificationError::MissingEvidence(obligation))
                if obligation == nested_add_obligation
        ));
    }
    for nested_multiply_obligation in nested_multiply_obligations {
        let mut missing_nested_multiply_proof =
            decode_proof_bundle(&proof).expect("decode shared proof");
        missing_nested_multiply_proof
            .evidence
            .retain(|evidence| evidence.obligation != nested_multiply_obligation);
        assert!(matches!(
            terminal_verifier::verify_module(
                &decode_module(&semantics).expect("decode shared semantics"),
                &missing_nested_multiply_proof,
                &AdmissionProfile::default(),
            ),
            Err(terminal_verifier::VerificationError::MissingEvidence(obligation))
                if obligation == nested_multiply_obligation
        ));
    }
    for affine_obligation in affine_obligations
        .iter()
        .chain(&zero_affine_obligations)
        .chain(&signed_affine_obligations)
        .chain(&affine_cast_obligations)
        .chain(&zero_affine_cast_obligations)
        .chain(&signed_affine_cast_obligations)
        .chain(&cast_then_affine_obligations)
        .chain(&zero_cast_then_affine_obligations)
        .chain(&signed_cast_then_affine_obligations)
    {
        let mut missing_affine_proof = decode_proof_bundle(&proof).expect("decode shared proof");
        missing_affine_proof
            .evidence
            .retain(|evidence| evidence.obligation != *affine_obligation);
        assert!(matches!(
            terminal_verifier::verify_module(
                &decode_module(&semantics).expect("decode shared semantics"),
                &missing_affine_proof,
                &AdmissionProfile::default(),
            ),
            Err(terminal_verifier::VerificationError::MissingEvidence(obligation))
                if obligation == *affine_obligation
        ));
    }
    let mut changed_middle_addend = decode_module(&semantics).expect("decode shared semantics");
    let changed_addend = changed_middle_addend
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            operation.result.scalar_ref().map(|result| result.id) == Some(middle_addend)
        })
        .expect("middle exact-add landed addend operation");
    changed_addend.kind = OperationKind::IntegerConstant {
        value: IntegerValue::Unsigned(2),
    };
    assert!(matches!(
        terminal_verifier::verify_module(
            &changed_middle_addend,
            &decode_proof_bundle(&proof).expect("decode unchanged shared proof"),
            &AdmissionProfile::default(),
        ),
        Err(terminal_verifier::VerificationError::RejectedEvidence {
            obligation,
            ..
        }) if obligation == nested_add_obligations[1]
    ));
    let mut changed_outer_addend = decode_module(&semantics).expect("decode shared semantics");
    let changed_addend = changed_outer_addend
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            operation.result.scalar_ref().map(|result| result.id) == Some(outer_addend)
        })
        .expect("outer exact-add landed addend operation");
    changed_addend.kind = OperationKind::IntegerConstant {
        value: IntegerValue::Unsigned(2),
    };
    assert!(matches!(
        terminal_verifier::verify_module(
            &changed_outer_addend,
            &decode_proof_bundle(&proof).expect("decode unchanged shared proof"),
            &AdmissionProfile::default(),
        ),
        Err(terminal_verifier::VerificationError::RejectedEvidence {
            obligation,
            ..
        }) if obligation == nested_add_obligations[2]
    ));
    let mut changed_add_bound = decode_module(&semantics).expect("decode shared semantics");
    let changed_entry = changed_add_bound.entry;
    let entry_contract = &mut changed_add_bound
        .machines
        .iter_mut()
        .find(|machine| machine.id == changed_entry)
        .expect("changed shared entry")
        .contract;
    let add_requirement = entry_contract
        .requires
        .iter()
        .position(|requirement| requirement == &add_upper_requirement)
        .expect("shared convergence retains the exact-add upper-bound premise");
    entry_contract.requires[add_requirement] = Proposition::LessOrEqual(
        ScalarTerm::value(entry.parameters[1].id, entry.parameters[1].scalar_type),
        ScalarTerm::integer(
            IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
            IntegerValue::Unsigned(253),
        )
        .unwrap(),
    );
    assert!(matches!(
        terminal_verifier::verify_module(
            &changed_add_bound,
            &decode_proof_bundle(&proof).expect("decode unchanged shared proof"),
            &AdmissionProfile::default(),
        ),
        Err(terminal_verifier::VerificationError::RejectedEvidence {
            obligation,
            ..
        }) if obligation == exact_add_obligation
    ));
    let mut missing_exact_subtract_proof =
        decode_proof_bundle(&proof).expect("decode shared proof");
    missing_exact_subtract_proof
        .evidence
        .retain(|evidence| evidence.obligation != exact_subtract_obligation);
    assert!(matches!(
        terminal_verifier::verify_module(
            &decode_module(&semantics).expect("decode shared semantics"),
            &missing_exact_subtract_proof,
            &AdmissionProfile::default(),
        ),
        Err(terminal_verifier::VerificationError::MissingEvidence(obligation))
            if obligation == exact_subtract_obligation
    ));
    let mut missing_exact_multiply_proof =
        decode_proof_bundle(&proof).expect("decode shared proof");
    missing_exact_multiply_proof
        .evidence
        .retain(|evidence| evidence.obligation != exact_multiply_obligation);
    assert!(matches!(
        terminal_verifier::verify_module(
            &decode_module(&semantics).expect("decode shared semantics"),
            &missing_exact_multiply_proof,
            &AdmissionProfile::default(),
        ),
        Err(terminal_verifier::VerificationError::MissingEvidence(obligation))
            if obligation == exact_multiply_obligation
    ));
    let mut missing_runtime_multiply_proof =
        decode_proof_bundle(&proof).expect("decode shared proof");
    missing_runtime_multiply_proof
        .evidence
        .retain(|evidence| evidence.obligation != runtime_exact_multiply_obligation);
    assert!(matches!(
        terminal_verifier::verify_module(
            &decode_module(&semantics).expect("decode shared semantics"),
            &missing_runtime_multiply_proof,
            &AdmissionProfile::default(),
        ),
        Err(terminal_verifier::VerificationError::MissingEvidence(obligation))
            if obligation == runtime_exact_multiply_obligation
    ));
    let mut changed_runtime_multiply_bound =
        decode_module(&semantics).expect("decode shared semantics");
    let changed_entry = changed_runtime_multiply_bound.entry;
    let entry_contract = &mut changed_runtime_multiply_bound
        .machines
        .iter_mut()
        .find(|machine| machine.id == changed_entry)
        .expect("changed shared entry")
        .contract;
    let runtime_multiply_requirement_position = entry_contract
        .requires
        .iter()
        .position(|requirement| requirement == &runtime_multiply_requirement)
        .expect("shared convergence retains the computed runtime-multiply bound");
    entry_contract.requires[runtime_multiply_requirement_position] = Proposition::LessOrEqual(
        ScalarTerm::value(entry.parameters[1].id, entry.parameters[1].scalar_type),
        ScalarTerm::exact_integer_divide(
            add_type,
            ScalarTerm::integer(add_type, IntegerValue::Unsigned(254)).unwrap(),
            ScalarTerm::value(entry.parameters[2].id, entry.parameters[2].scalar_type),
        )
        .unwrap(),
    );
    assert!(matches!(
        terminal_verifier::verify_module(
            &changed_runtime_multiply_bound,
            &decode_proof_bundle(&proof).expect("decode unchanged shared proof"),
            &AdmissionProfile::default(),
        ),
        Err(terminal_verifier::VerificationError::RejectedEvidence {
            obligation,
            ..
        }) if obligation == runtime_exact_multiply_obligation
    ));
    for obligation in &runtime_signed_multiply_obligations {
        let mut missing_runtime_signed_multiply_proof =
            decode_proof_bundle(&proof).expect("decode shared proof");
        missing_runtime_signed_multiply_proof
            .evidence
            .retain(|evidence| evidence.obligation != *obligation);
        assert!(matches!(
            terminal_verifier::verify_module(
                &decode_module(&semantics).expect("decode shared semantics"),
                &missing_runtime_signed_multiply_proof,
                &AdmissionProfile::default(),
            ),
            Err(terminal_verifier::VerificationError::MissingEvidence(missing))
                if missing == *obligation
        ));
    }
    let changed_positive_multiply_requirement = Proposition::LessOrEqual(
        ScalarTerm::exact_integer_divide(
            signed_arithmetic_type,
            ScalarTerm::integer(signed_arithmetic_type, IntegerValue::Signed(-127)).unwrap(),
            ScalarTerm::value(entry.parameters[6].id, entry.parameters[6].scalar_type),
        )
        .unwrap(),
        ScalarTerm::value(entry.parameters[5].id, entry.parameters[5].scalar_type),
    );
    let changed_negative_multiply_requirement = Proposition::LessOrEqual(
        ScalarTerm::exact_integer_divide(
            signed_arithmetic_type,
            ScalarTerm::integer(signed_arithmetic_type, IntegerValue::Signed(126)).unwrap(),
            ScalarTerm::value(entry.parameters[7].id, entry.parameters[7].scalar_type),
        )
        .unwrap(),
        ScalarTerm::value(entry.parameters[5].id, entry.parameters[5].scalar_type),
    );
    for (original, replacement, obligation) in [
        (
            &runtime_signed_positive_multiply_lower_requirement,
            changed_positive_multiply_requirement,
            runtime_signed_multiply_obligations[0],
        ),
        (
            &runtime_signed_negative_multiply_lower_requirement,
            changed_negative_multiply_requirement,
            runtime_signed_multiply_obligations[1],
        ),
    ] {
        let mut changed_runtime_signed_multiply_bound =
            decode_module(&semantics).expect("decode shared semantics");
        let changed_entry = changed_runtime_signed_multiply_bound.entry;
        let entry_contract = &mut changed_runtime_signed_multiply_bound
            .machines
            .iter_mut()
            .find(|machine| machine.id == changed_entry)
            .expect("changed shared entry")
            .contract;
        let position = entry_contract
            .requires
            .iter()
            .position(|requirement| requirement == original)
            .expect("shared convergence retains each signed quotient runtime-multiply bound");
        entry_contract.requires[position] = replacement;
        assert!(matches!(
            terminal_verifier::verify_module(
                &changed_runtime_signed_multiply_bound,
                &decode_proof_bundle(&proof).expect("decode unchanged shared proof"),
                &AdmissionProfile::default(),
            ),
            Err(terminal_verifier::VerificationError::RejectedEvidence {
                obligation: rejected,
                ..
            }) if rejected == obligation
        ));
    }
    let mut changed_exact_bound = decode_module(&semantics).expect("decode shared semantics");
    let changed_entry = changed_exact_bound.entry;
    let entry_contract = &mut changed_exact_bound
        .machines
        .iter_mut()
        .find(|machine| machine.id == changed_entry)
        .expect("changed shared entry")
        .contract;
    let exact_requirement = entry_contract
        .requires
        .iter()
        .position(|requirement| requirement == &exact_upper_requirement)
        .expect("shared convergence retains the subtract/multiply upper-bound premise");
    entry_contract.requires[exact_requirement] = Proposition::LessOrEqual(
        ScalarTerm::value(entry.parameters[1].id, entry.parameters[1].scalar_type),
        ScalarTerm::integer(
            IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
            IntegerValue::Unsigned(126),
        )
        .unwrap(),
    );
    assert!(matches!(
        terminal_verifier::verify_module(
            &changed_exact_bound,
            &decode_proof_bundle(&proof).expect("decode unchanged shared proof"),
            &AdmissionProfile::default(),
        ),
        Err(terminal_verifier::VerificationError::RejectedEvidence {
            obligation,
            ..
        }) if obligation == exact_subtract_obligation || obligation == exact_multiply_obligation
    ));
    for obligation in [
        exact_divide_obligation,
        exact_remainder_obligation,
        runtime_exact_divide_obligation,
        runtime_exact_remainder_obligation,
    ] {
        let mut missing_proof = decode_proof_bundle(&proof).expect("decode shared proof");
        missing_proof
            .evidence
            .retain(|evidence| evidence.obligation != obligation);
        assert!(matches!(
            terminal_verifier::verify_module(
                &decode_module(&semantics).expect("decode shared semantics"),
                &missing_proof,
                &AdmissionProfile::default(),
            ),
            Err(terminal_verifier::VerificationError::MissingEvidence(missing))
            if missing == obligation
        ));
    }
    let mut changed_divisor_bound = decode_module(&semantics).expect("decode shared semantics");
    let changed_entry = changed_divisor_bound.entry;
    let entry_contract = &mut changed_divisor_bound
        .machines
        .iter_mut()
        .find(|machine| machine.id == changed_entry)
        .expect("changed shared entry")
        .contract;
    let divisor_requirement = entry_contract
        .requires
        .iter()
        .position(|requirement| requirement == &divisor_lower_requirement)
        .expect("shared convergence retains the runtime-divisor lower-bound premise");
    entry_contract.requires[divisor_requirement] = Proposition::LessOrEqual(
        ScalarTerm::integer(
            IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
            IntegerValue::Unsigned(2),
        )
        .unwrap(),
        ScalarTerm::value(entry.parameters[2].id, entry.parameters[2].scalar_type),
    );
    assert!(matches!(
        terminal_verifier::verify_module(
            &changed_divisor_bound,
            &decode_proof_bundle(&proof).expect("decode unchanged shared proof"),
            &AdmissionProfile::default(),
        ),
        Err(terminal_verifier::VerificationError::RejectedEvidence {
            obligation,
            ..
        }) if obligation == runtime_exact_divide_obligation
            || obligation == runtime_exact_remainder_obligation
            || obligation == runtime_exact_multiply_obligation
    ));
    let mut changed_signed_divisor_bound =
        decode_module(&semantics).expect("decode shared semantics");
    let changed_entry = changed_signed_divisor_bound.entry;
    let entry_contract = &mut changed_signed_divisor_bound
        .machines
        .iter_mut()
        .find(|machine| machine.id == changed_entry)
        .expect("changed shared entry")
        .contract;
    let signed_divisor_requirement = entry_contract
        .requires
        .iter()
        .position(|requirement| requirement == &signed_divisor_lower_requirement)
        .expect("shared convergence retains the signed runtime-divisor lower-bound premise");
    entry_contract.requires[signed_divisor_requirement] = Proposition::LessOrEqual(
        ScalarTerm::integer(
            IntegerType::new(IntegerSign::Signed, 8).unwrap(),
            IntegerValue::Signed(2),
        )
        .unwrap(),
        ScalarTerm::value(entry.parameters[6].id, entry.parameters[6].scalar_type),
    );
    assert!(matches!(
        terminal_verifier::verify_module(
            &changed_signed_divisor_bound,
            &decode_proof_bundle(&proof).expect("decode unchanged shared proof"),
            &AdmissionProfile::default(),
        ),
        Err(terminal_verifier::VerificationError::RejectedEvidence {
            obligation,
            ..
        }) if runtime_signed_division_obligations.contains(&obligation)
            || obligation == runtime_signed_multiply_obligations[0]
    ));
    let mut changed_negative_divisor_bound =
        decode_module(&semantics).expect("decode shared semantics");
    let changed_entry = changed_negative_divisor_bound.entry;
    let entry_contract = &mut changed_negative_divisor_bound
        .machines
        .iter_mut()
        .find(|machine| machine.id == changed_entry)
        .expect("changed shared entry")
        .contract;
    let negative_divisor_requirement = entry_contract
        .requires
        .iter()
        .position(|requirement| requirement == &negative_divisor_upper_requirement)
        .expect("shared convergence retains the negative runtime-divisor upper-bound premise");
    entry_contract.requires[negative_divisor_requirement] = Proposition::LessOrEqual(
        ScalarTerm::value(entry.parameters[7].id, entry.parameters[7].scalar_type),
        ScalarTerm::integer(
            IntegerType::new(IntegerSign::Signed, 8).unwrap(),
            IntegerValue::Signed(-3),
        )
        .unwrap(),
    );
    assert!(matches!(
        terminal_verifier::verify_module(
            &changed_negative_divisor_bound,
            &decode_proof_bundle(&proof).expect("decode unchanged shared proof"),
            &AdmissionProfile::default(),
        ),
        Err(terminal_verifier::VerificationError::RejectedEvidence {
            obligation,
            ..
        }) if runtime_negative_signed_division_obligations.contains(&obligation)
            || obligation == runtime_signed_multiply_obligations[1]
    ));
    let mut changed_bounded_negative_divisor_bound =
        decode_module(&semantics).expect("decode shared semantics");
    let changed_entry = changed_bounded_negative_divisor_bound.entry;
    let entry_contract = &mut changed_bounded_negative_divisor_bound
        .machines
        .iter_mut()
        .find(|machine| machine.id == changed_entry)
        .expect("changed shared entry")
        .contract;
    let bounded_negative_divisor_requirement = entry_contract
        .requires
        .iter()
        .position(|requirement| requirement == &bounded_negative_divisor_upper_requirement)
        .expect("shared convergence retains the jointly bounded runtime-divisor premise");
    entry_contract.requires[bounded_negative_divisor_requirement] = Proposition::LessOrEqual(
        ScalarTerm::value(entry.parameters[8].id, entry.parameters[8].scalar_type),
        ScalarTerm::integer(
            IntegerType::new(IntegerSign::Signed, 8).unwrap(),
            IntegerValue::Signed(-2),
        )
        .unwrap(),
    );
    assert!(matches!(
        terminal_verifier::verify_module(
            &changed_bounded_negative_divisor_bound,
            &decode_proof_bundle(&proof).expect("decode unchanged shared proof"),
            &AdmissionProfile::default(),
        ),
        Err(terminal_verifier::VerificationError::RejectedEvidence {
            obligation,
            ..
        }) if runtime_bounded_negative_signed_division_obligations.contains(&obligation)
    ));
    let mut missing_runtime_add_proof = decode_proof_bundle(&proof).expect("decode shared proof");
    missing_runtime_add_proof
        .evidence
        .retain(|evidence| evidence.obligation != runtime_exact_add_obligation);
    assert!(matches!(
        terminal_verifier::verify_module(
            &decode_module(&semantics).expect("decode shared semantics"),
            &missing_runtime_add_proof,
            &AdmissionProfile::default(),
        ),
        Err(terminal_verifier::VerificationError::MissingEvidence(obligation))
            if obligation == runtime_exact_add_obligation
    ));
    let mut changed_runtime_add_bound = decode_module(&semantics).expect("decode shared semantics");
    let changed_entry = changed_runtime_add_bound.entry;
    let entry_contract = &mut changed_runtime_add_bound
        .machines
        .iter_mut()
        .find(|machine| machine.id == changed_entry)
        .expect("changed shared entry")
        .contract;
    let runtime_add_requirement_position = entry_contract
        .requires
        .iter()
        .position(|requirement| requirement == &runtime_add_requirement)
        .expect("shared convergence retains the computed runtime-add bound");
    entry_contract.requires[runtime_add_requirement_position] = Proposition::LessOrEqual(
        ScalarTerm::value(entry.parameters[9].id, entry.parameters[9].scalar_type),
        ScalarTerm::exact_integer_subtract(
            add_type,
            ScalarTerm::integer(add_type, IntegerValue::Unsigned(254)).unwrap(),
            ScalarTerm::value(entry.parameters[10].id, entry.parameters[10].scalar_type),
        )
        .unwrap(),
    );
    assert!(matches!(
        terminal_verifier::verify_module(
            &changed_runtime_add_bound,
            &decode_proof_bundle(&proof).expect("decode unchanged shared proof"),
            &AdmissionProfile::default(),
        ),
        Err(terminal_verifier::VerificationError::RejectedEvidence {
            obligation,
            ..
        }) if obligation == runtime_exact_add_obligation
    ));
    let nested_bitwise_add_obligation = bitwise_not_exact_add_obligations[0];
    let mut missing_nested_bitwise_add_proof =
        decode_proof_bundle(&proof).expect("decode shared proof");
    missing_nested_bitwise_add_proof
        .evidence
        .retain(|evidence| evidence.obligation != nested_bitwise_add_obligation);
    assert!(matches!(
        terminal_verifier::verify_module(
            &decode_module(&semantics).expect("decode shared semantics"),
            &missing_nested_bitwise_add_proof,
            &AdmissionProfile::default(),
        ),
        Err(terminal_verifier::VerificationError::MissingEvidence(obligation))
            if obligation == nested_bitwise_add_obligation
    ));
    let mut changed_nested_bitwise_add_bound =
        decode_module(&semantics).expect("decode shared semantics");
    let changed_entry = changed_nested_bitwise_add_bound.entry;
    let entry_contract = &mut changed_nested_bitwise_add_bound
        .machines
        .iter_mut()
        .find(|machine| machine.id == changed_entry)
        .expect("changed shared entry")
        .contract;
    let nested_bitwise_add_requirement = entry_contract
        .requires
        .iter()
        .position(|requirement| requirement == &bitwise_not_exact_add_requirement)
        .expect("shared convergence retains the nested bitwise exact-add bound");
    entry_contract.requires[nested_bitwise_add_requirement] = Proposition::LessOrEqual(
        ScalarTerm::value(entry.parameters[1].id, entry.parameters[1].scalar_type),
        unsigned_term(8, 253),
    );
    assert!(matches!(
        terminal_verifier::verify_module(
            &changed_nested_bitwise_add_bound,
            &decode_proof_bundle(&proof).expect("decode unchanged shared proof"),
            &AdmissionProfile::default(),
        ),
        Err(terminal_verifier::VerificationError::RejectedEvidence {
            obligation,
            ..
        }) if obligation == nested_bitwise_add_obligation
            || obligation == nested_add_obligations[2]
    ));
    for nested_subtract_obligation in nested_subtract_obligations {
        let mut missing_nested_subtract_proof =
            decode_proof_bundle(&proof).expect("decode shared proof");
        missing_nested_subtract_proof
            .evidence
            .retain(|evidence| evidence.obligation != nested_subtract_obligation);
        assert!(matches!(
            terminal_verifier::verify_module(
                &decode_module(&semantics).expect("decode shared semantics"),
                &missing_nested_subtract_proof,
                &AdmissionProfile::default(),
            ),
            Err(terminal_verifier::VerificationError::MissingEvidence(obligation))
                if obligation == nested_subtract_obligation
        ));
    }
    for mixed_add_subtract_obligation in mixed_add_subtract_obligations {
        let mut missing_mixed_add_subtract_proof =
            decode_proof_bundle(&proof).expect("decode shared proof");
        missing_mixed_add_subtract_proof
            .evidence
            .retain(|evidence| evidence.obligation != mixed_add_subtract_obligation);
        assert!(matches!(
            terminal_verifier::verify_module(
                &decode_module(&semantics).expect("decode shared semantics"),
                &missing_mixed_add_subtract_proof,
                &AdmissionProfile::default(),
            ),
            Err(terminal_verifier::VerificationError::MissingEvidence(obligation))
                if obligation == mixed_add_subtract_obligation
        ));
    }
    for offset_chain_cast_obligation in offset_chain_cast_obligations {
        let mut missing_offset_chain_cast_proof =
            decode_proof_bundle(&proof).expect("decode shared proof");
        missing_offset_chain_cast_proof
            .evidence
            .retain(|evidence| evidence.obligation != offset_chain_cast_obligation);
        assert!(matches!(
            terminal_verifier::verify_module(
                &decode_module(&semantics).expect("decode shared semantics"),
                &missing_offset_chain_cast_proof,
                &AdmissionProfile::default(),
            ),
            Err(terminal_verifier::VerificationError::MissingEvidence(obligation))
                if obligation == offset_chain_cast_obligation
        ));
    }
    for cast_then_offset_obligation in cast_then_add_obligations
        .into_iter()
        .chain(cast_then_subtract_obligations)
    {
        let mut missing_cast_then_offset_proof =
            decode_proof_bundle(&proof).expect("decode shared proof");
        missing_cast_then_offset_proof
            .evidence
            .retain(|evidence| evidence.obligation != cast_then_offset_obligation);
        assert!(matches!(
            terminal_verifier::verify_module(
                &decode_module(&semantics).expect("decode shared semantics"),
                &missing_cast_then_offset_proof,
                &AdmissionProfile::default(),
            ),
            Err(terminal_verifier::VerificationError::MissingEvidence(obligation))
                if obligation == cast_then_offset_obligation
        ));
    }
    for finite_cast_then_offset_obligation in finite_cast_then_offset_obligations
        .into_iter()
        .chain(cancelling_cast_then_offset_obligations)
    {
        let mut missing_finite_cast_then_offset_proof =
            decode_proof_bundle(&proof).expect("decode shared proof");
        missing_finite_cast_then_offset_proof
            .evidence
            .retain(|evidence| evidence.obligation != finite_cast_then_offset_obligation);
        assert!(matches!(
            terminal_verifier::verify_module(
                &decode_module(&semantics).expect("decode shared semantics"),
                &missing_finite_cast_then_offset_proof,
                &AdmissionProfile::default(),
            ),
            Err(terminal_verifier::VerificationError::MissingEvidence(obligation))
                if obligation == finite_cast_then_offset_obligation
        ));
    }
    for cast_then_multiply_obligation in cast_then_multiply_obligations
        .into_iter()
        .chain(zero_cast_then_multiply_obligations)
    {
        let mut missing_cast_then_multiply_proof =
            decode_proof_bundle(&proof).expect("decode shared proof");
        missing_cast_then_multiply_proof
            .evidence
            .retain(|evidence| evidence.obligation != cast_then_multiply_obligation);
        assert!(matches!(
            terminal_verifier::verify_module(
                &decode_module(&semantics).expect("decode shared semantics"),
                &missing_cast_then_multiply_proof,
                &AdmissionProfile::default(),
            ),
            Err(terminal_verifier::VerificationError::MissingEvidence(obligation))
                if obligation == cast_then_multiply_obligation
        ));
    }
    for multiply_chain_then_cast_obligation in multiply_chain_then_cast_obligations
        .into_iter()
        .chain(zero_multiply_chain_then_cast_obligations)
    {
        let mut missing_multiply_chain_then_cast_proof =
            decode_proof_bundle(&proof).expect("decode shared proof");
        missing_multiply_chain_then_cast_proof
            .evidence
            .retain(|evidence| evidence.obligation != multiply_chain_then_cast_obligation);
        assert!(matches!(
            terminal_verifier::verify_module(
                &decode_module(&semantics).expect("decode shared semantics"),
                &missing_multiply_chain_then_cast_proof,
                &AdmissionProfile::default(),
            ),
            Err(terminal_verifier::VerificationError::MissingEvidence(obligation))
                if obligation == multiply_chain_then_cast_obligation
        ));
    }
    for nested_divide_remainder_obligation in nested_divide_remainder_obligations {
        let mut missing_nested_divide_remainder_proof =
            decode_proof_bundle(&proof).expect("decode shared proof");
        missing_nested_divide_remainder_proof
            .evidence
            .retain(|evidence| evidence.obligation != nested_divide_remainder_obligation);
        assert!(matches!(
            terminal_verifier::verify_module(
                &decode_module(&semantics).expect("decode shared semantics"),
                &missing_nested_divide_remainder_proof,
                &AdmissionProfile::default(),
            ),
            Err(terminal_verifier::VerificationError::MissingEvidence(obligation))
                if obligation == nested_divide_remainder_obligation
        ));
    }
    for cast_then_divide_remainder_obligation in cast_then_divide_remainder_obligations
        .into_iter()
        .chain(signed_cast_then_divide_remainder_obligations)
        .chain(cross_cast_then_divide_remainder_obligations)
    {
        let mut missing_cast_then_divide_remainder_proof =
            decode_proof_bundle(&proof).expect("decode shared proof");
        missing_cast_then_divide_remainder_proof
            .evidence
            .retain(|evidence| evidence.obligation != cast_then_divide_remainder_obligation);
        assert!(matches!(
            terminal_verifier::verify_module(
                &decode_module(&semantics).expect("decode shared semantics"),
                &missing_cast_then_divide_remainder_proof,
                &AdmissionProfile::default(),
            ),
            Err(terminal_verifier::VerificationError::MissingEvidence(obligation))
                if obligation == cast_then_divide_remainder_obligation
        ));
    }
    for runtime_divisor_chain_obligation in direct_unsigned_runtime_divisor_obligations
        .into_iter()
        .chain(direct_signed_positive_runtime_divisor_obligations)
        .chain(direct_signed_negative_runtime_divisor_obligations)
        .chain(post_cast_unsigned_runtime_divisor_obligations)
        .chain(post_cast_signed_positive_runtime_divisor_obligations)
        .chain(post_cast_signed_negative_runtime_divisor_obligations)
    {
        let mut missing_runtime_divisor_chain_proof =
            decode_proof_bundle(&proof).expect("decode shared proof");
        missing_runtime_divisor_chain_proof
            .evidence
            .retain(|evidence| evidence.obligation != runtime_divisor_chain_obligation);
        assert!(matches!(
            terminal_verifier::verify_module(
                &decode_module(&semantics).expect("decode shared semantics"),
                &missing_runtime_divisor_chain_proof,
                &AdmissionProfile::default(),
            ),
            Err(terminal_verifier::VerificationError::MissingEvidence(obligation))
                if obligation == runtime_divisor_chain_obligation
        ));
    }
    for divide_remainder_chain_cast_obligation in divide_chain_cast_obligations
        .into_iter()
        .chain(mixed_divide_remainder_cast_obligations)
        .chain(signed_remainder_cast_obligations)
        .chain(cross_remainder_cast_obligations)
    {
        let mut missing_divide_remainder_chain_cast_proof =
            decode_proof_bundle(&proof).expect("decode shared proof");
        missing_divide_remainder_chain_cast_proof
            .evidence
            .retain(|evidence| evidence.obligation != divide_remainder_chain_cast_obligation);
        assert!(matches!(
            terminal_verifier::verify_module(
                &decode_module(&semantics).expect("decode shared semantics"),
                &missing_divide_remainder_chain_cast_proof,
                &AdmissionProfile::default(),
            ),
            Err(terminal_verifier::VerificationError::MissingEvidence(obligation))
                if obligation == divide_remainder_chain_cast_obligation
        ));
    }
    for nested_shift_right_obligation in nested_shift_right_obligations {
        let mut missing_nested_shift_right_proof =
            decode_proof_bundle(&proof).expect("decode shared proof");
        missing_nested_shift_right_proof
            .evidence
            .retain(|evidence| evidence.obligation != nested_shift_right_obligation);
        assert!(matches!(
            terminal_verifier::verify_module(
                &decode_module(&semantics).expect("decode shared semantics"),
                &missing_nested_shift_right_proof,
                &AdmissionProfile::default(),
            ),
            Err(terminal_verifier::VerificationError::MissingEvidence(obligation))
                if obligation == nested_shift_right_obligation
        ));
    }
    for shift_right_then_cast_obligation in shift_right_then_cast_obligations
        .into_iter()
        .chain(zero_shift_right_then_cast_obligations)
    {
        let mut missing_shift_right_then_cast_proof =
            decode_proof_bundle(&proof).expect("decode shared proof");
        missing_shift_right_then_cast_proof
            .evidence
            .retain(|evidence| evidence.obligation != shift_right_then_cast_obligation);
        assert!(matches!(
            terminal_verifier::verify_module(
                &decode_module(&semantics).expect("decode shared semantics"),
                &missing_shift_right_then_cast_proof,
                &AdmissionProfile::default(),
            ),
            Err(terminal_verifier::VerificationError::MissingEvidence(obligation))
                if obligation == shift_right_then_cast_obligation
        ));
    }
    for cast_then_shift_right_obligation in cast_then_shift_right_obligations
        .into_iter()
        .chain(signed_cast_then_shift_right_obligations)
        .chain(cross_cast_then_shift_right_obligations)
    {
        let mut missing_cast_then_shift_right_proof =
            decode_proof_bundle(&proof).expect("decode shared proof");
        missing_cast_then_shift_right_proof
            .evidence
            .retain(|evidence| evidence.obligation != cast_then_shift_right_obligation);
        assert!(matches!(
            terminal_verifier::verify_module(
                &decode_module(&semantics).expect("decode shared semantics"),
                &missing_cast_then_shift_right_proof,
                &AdmissionProfile::default(),
            ),
            Err(terminal_verifier::VerificationError::MissingEvidence(obligation))
                if obligation == cast_then_shift_right_obligation
        ));
    }
    for nested_shift_left_obligation in nested_shift_left_obligations {
        let mut missing_nested_shift_left_proof =
            decode_proof_bundle(&proof).expect("decode shared proof");
        missing_nested_shift_left_proof
            .evidence
            .retain(|evidence| evidence.obligation != nested_shift_left_obligation);
        assert!(matches!(
            terminal_verifier::verify_module(
                &decode_module(&semantics).expect("decode shared semantics"),
                &missing_nested_shift_left_proof,
                &AdmissionProfile::default(),
            ),
            Err(terminal_verifier::VerificationError::MissingEvidence(obligation))
                if obligation == nested_shift_left_obligation
        ));
    }
    for cast_then_shift_left_obligation in cast_then_shift_left_obligations {
        let mut missing_cast_then_shift_left_proof =
            decode_proof_bundle(&proof).expect("decode shared proof");
        missing_cast_then_shift_left_proof
            .evidence
            .retain(|evidence| evidence.obligation != cast_then_shift_left_obligation);
        assert!(matches!(
            terminal_verifier::verify_module(
                &decode_module(&semantics).expect("decode shared semantics"),
                &missing_cast_then_shift_left_proof,
                &AdmissionProfile::default(),
            ),
            Err(terminal_verifier::VerificationError::MissingEvidence(obligation))
                if obligation == cast_then_shift_left_obligation
        ));
    }
    for shift_left_then_cast_obligation in shift_left_then_cast_obligations
        .into_iter()
        .chain(zero_shift_then_cast_obligations)
    {
        let mut missing_shift_left_then_cast_proof =
            decode_proof_bundle(&proof).expect("decode shared proof");
        missing_shift_left_then_cast_proof
            .evidence
            .retain(|evidence| evidence.obligation != shift_left_then_cast_obligation);
        assert!(matches!(
            terminal_verifier::verify_module(
                &decode_module(&semantics).expect("decode shared semantics"),
                &missing_shift_left_then_cast_proof,
                &AdmissionProfile::default(),
            ),
            Err(terminal_verifier::VerificationError::MissingEvidence(obligation))
                if obligation == shift_left_then_cast_obligation
        ));
    }
    let mut missing_widen_exact_subtract_proof =
        decode_proof_bundle(&proof).expect("decode shared proof");
    missing_widen_exact_subtract_proof
        .evidence
        .retain(|evidence| evidence.obligation != widen_exact_subtract_obligation);
    assert!(matches!(
        terminal_verifier::verify_module(
            &decode_module(&semantics).expect("decode shared semantics"),
            &missing_widen_exact_subtract_proof,
            &AdmissionProfile::default(),
        ),
        Err(terminal_verifier::VerificationError::MissingEvidence(obligation))
            if obligation == widen_exact_subtract_obligation
    ));
    let mut changed_middle_subtrahend = decode_module(&semantics).expect("decode shared semantics");
    let changed_subtrahend = changed_middle_subtrahend
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            operation.result.scalar_ref().map(|result| result.id) == Some(middle_subtrahend)
        })
        .expect("middle exact-subtract landed subtrahend operation");
    changed_subtrahend.kind = OperationKind::IntegerConstant {
        value: IntegerValue::Unsigned(2),
    };
    assert!(matches!(
        terminal_verifier::verify_module(
            &changed_middle_subtrahend,
            &decode_proof_bundle(&proof).expect("decode unchanged shared proof"),
            &AdmissionProfile::default(),
        ),
        Err(terminal_verifier::VerificationError::RejectedEvidence {
            obligation,
            ..
        }) if obligation == nested_subtract_obligations[1]
    ));
    let mut changed_mixed_subtrahend = decode_module(&semantics).expect("decode shared semantics");
    let changed_subtrahend = changed_mixed_subtrahend
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            operation.result.scalar_ref().map(|result| result.id) == Some(mixed_subtrahend)
        })
        .expect("mixed exact-add/subtract landed subtrahend operation");
    changed_subtrahend.kind = OperationKind::IntegerConstant {
        value: IntegerValue::Unsigned(4),
    };
    assert!(matches!(
        terminal_verifier::verify_module(
            &changed_mixed_subtrahend,
            &decode_proof_bundle(&proof).expect("decode unchanged shared proof"),
            &AdmissionProfile::default(),
        ),
        Err(terminal_verifier::VerificationError::RejectedEvidence {
            obligation,
            ..
        }) if obligation == mixed_add_subtract_obligations[1]
    ));
    let mut changed_offset_cast_subtrahend =
        decode_module(&semantics).expect("decode shared semantics");
    let changed_subtrahend = changed_offset_cast_subtrahend
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            operation.result.scalar_ref().map(|result| result.id)
                == Some(offset_chain_cast_subtrahend)
        })
        .expect("offset-chain exact-cast landed subtrahend operation");
    changed_subtrahend.kind = OperationKind::IntegerConstant {
        value: IntegerValue::Unsigned(4),
    };
    assert!(matches!(
        terminal_verifier::verify_module(
            &changed_offset_cast_subtrahend,
            &decode_proof_bundle(&proof).expect("decode unchanged shared proof"),
            &AdmissionProfile::default(),
        ),
        Err(terminal_verifier::VerificationError::RejectedEvidence {
            obligation,
            ..
        }) if offset_chain_cast_obligations.contains(&obligation)
    ));
    let mut changed_cast_then_add_literal =
        decode_module(&semantics).expect("decode shared semantics");
    let changed_literal = changed_cast_then_add_literal
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            operation.result.scalar_ref().map(|result| result.id) == Some(cast_then_add_literal)
        })
        .expect("cast-then-add landed literal operation");
    changed_literal.kind = OperationKind::IntegerConstant {
        value: IntegerValue::Unsigned(6),
    };
    assert!(matches!(
        terminal_verifier::verify_module(
            &changed_cast_then_add_literal,
            &decode_proof_bundle(&proof).expect("decode unchanged shared proof"),
            &AdmissionProfile::default(),
        ),
        Err(terminal_verifier::VerificationError::RejectedEvidence {
            obligation,
            ..
        }) if cast_then_add_obligations.contains(&obligation)
    ));
    let mut changed_finite_middle_literal =
        decode_module(&semantics).expect("decode shared semantics");
    let changed_literal = changed_finite_middle_literal
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            operation.result.scalar_ref().map(|result| result.id) == Some(finite_middle_literal)
        })
        .expect("finite cast-then-offset middle landed literal operation");
    changed_literal.kind = OperationKind::IntegerConstant {
        value: IntegerValue::Unsigned(4),
    };
    assert!(matches!(
        terminal_verifier::verify_module(
            &changed_finite_middle_literal,
            &decode_proof_bundle(&proof).expect("decode unchanged shared proof"),
            &AdmissionProfile::default(),
        ),
        Err(terminal_verifier::VerificationError::RejectedEvidence {
            obligation,
            ..
        }) if finite_cast_then_offset_obligations.contains(&obligation)
    ));
    let mut changed_cast_then_multiply_factor =
        decode_module(&semantics).expect("decode shared semantics");
    let changed_factor = changed_cast_then_multiply_factor
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            operation.result.scalar_ref().map(|result| result.id)
                == Some(cast_then_multiply_outer_factor)
        })
        .expect("post-cast multiply landed outer factor operation");
    changed_factor.kind = OperationKind::IntegerConstant {
        value: IntegerValue::Unsigned(4),
    };
    assert!(matches!(
        terminal_verifier::verify_module(
            &changed_cast_then_multiply_factor,
            &decode_proof_bundle(&proof).expect("decode unchanged shared proof"),
            &AdmissionProfile::default(),
        ),
        Err(terminal_verifier::VerificationError::RejectedEvidence {
            obligation,
            ..
        }) if cast_then_multiply_obligations.contains(&obligation)
    ));
    let mut changed_multiply_chain_then_cast_factor =
        decode_module(&semantics).expect("decode shared semantics");
    let changed_factor = changed_multiply_chain_then_cast_factor
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            operation.result.scalar_ref().map(|result| result.id)
                == Some(multiply_chain_then_cast_outer_factor)
        })
        .expect("pre-cast multiply landed outer factor operation");
    changed_factor.kind = OperationKind::IntegerConstant {
        value: IntegerValue::Unsigned(4),
    };
    assert!(matches!(
        terminal_verifier::verify_module(
            &changed_multiply_chain_then_cast_factor,
            &decode_proof_bundle(&proof).expect("decode unchanged shared proof"),
            &AdmissionProfile::default(),
        ),
        Err(terminal_verifier::VerificationError::RejectedEvidence {
            obligation,
            ..
        }) if multiply_chain_then_cast_obligations.contains(&obligation)
    ));
    let mut changed_middle_divisor = decode_module(&semantics).expect("decode shared semantics");
    let changed_divisor = changed_middle_divisor
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            operation.result.scalar_ref().map(|result| result.id) == Some(middle_divisor)
        })
        .expect("middle exact-remainder landed divisor operation");
    changed_divisor.kind = OperationKind::IntegerConstant {
        value: IntegerValue::Unsigned(0),
    };
    assert!(matches!(
        terminal_verifier::verify_module(
            &changed_middle_divisor,
            &decode_proof_bundle(&proof).expect("decode unchanged shared proof"),
            &AdmissionProfile::default(),
        ),
        Err(terminal_verifier::VerificationError::RejectedEvidence {
            obligation,
            ..
        }) if obligation == nested_divide_remainder_obligations[1]
    ));
    let mut changed_cast_then_divide_remainder_divisor =
        decode_module(&semantics).expect("decode shared semantics");
    let changed_divisor = changed_cast_then_divide_remainder_divisor
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            operation.result.scalar_ref().map(|result| result.id)
                == Some(cast_then_divide_remainder_middle_divisor)
        })
        .expect("post-cast divide/remainder landed divisor operation");
    changed_divisor.kind = OperationKind::IntegerConstant {
        value: IntegerValue::Unsigned(0),
    };
    assert!(matches!(
        terminal_verifier::verify_module(
            &changed_cast_then_divide_remainder_divisor,
            &decode_proof_bundle(&proof).expect("decode unchanged shared proof"),
            &AdmissionProfile::default(),
        ),
        Err(terminal_verifier::VerificationError::RejectedEvidence {
            obligation,
            ..
        }) if cast_then_divide_remainder_obligations.contains(&obligation)
    ));
    let mut changed_divide_remainder_chain_cast_divisor =
        decode_module(&semantics).expect("decode shared semantics");
    let changed_divisor = changed_divide_remainder_chain_cast_divisor
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            operation.result.scalar_ref().map(|result| result.id) == Some(mixed_cast_divisor)
        })
        .expect("pre-cast divide/remainder landed divisor operation");
    changed_divisor.kind = OperationKind::IntegerConstant {
        value: IntegerValue::Unsigned(300),
    };
    assert!(matches!(
        terminal_verifier::verify_module(
            &changed_divide_remainder_chain_cast_divisor,
            &decode_proof_bundle(&proof).expect("decode unchanged shared proof"),
            &AdmissionProfile::default(),
        ),
        Err(terminal_verifier::VerificationError::RejectedEvidence {
            obligation,
            ..
        }) if mixed_divide_remainder_cast_obligations.contains(&obligation)
    ));
    let mut changed_middle_factor = decode_module(&semantics).expect("decode shared semantics");
    let changed_factor = changed_middle_factor
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            operation.result.scalar_ref().map(|result| result.id) == Some(middle_factor)
        })
        .expect("middle exact-multiply landed factor operation");
    changed_factor.kind = OperationKind::IntegerConstant {
        value: IntegerValue::Unsigned(4),
    };
    assert!(matches!(
        terminal_verifier::verify_module(
            &changed_middle_factor,
            &decode_proof_bundle(&proof).expect("decode unchanged shared proof"),
            &AdmissionProfile::default(),
        ),
        Err(terminal_verifier::VerificationError::RejectedEvidence {
            obligation,
            ..
        }) if obligation == nested_multiply_obligations[1]
    ));
    let mut changed_affine_factor = decode_module(&semantics).expect("decode shared semantics");
    let changed_factor = changed_affine_factor
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            operation.result.scalar_ref().map(|result| result.id) == Some(affine_factor)
        })
        .expect("mixed affine chain retains its landed factor operation");
    changed_factor.kind = OperationKind::IntegerConstant {
        value: IntegerValue::Unsigned(3),
    };
    assert!(matches!(
        terminal_verifier::verify_module(
            &changed_affine_factor,
            &decode_proof_bundle(&proof).expect("decode unchanged shared proof"),
            &AdmissionProfile::default(),
        ),
        Err(terminal_verifier::VerificationError::RejectedEvidence {
            obligation,
            ..
        }) if affine_obligations.contains(&obligation)
    ));
    let mut changed_affine_cast_factor =
        decode_module(&semantics).expect("decode shared semantics");
    let changed_factor = changed_affine_cast_factor
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            operation.result.scalar_ref().map(|result| result.id) == Some(affine_cast_factor)
        })
        .expect("pre-cast affine chain retains its landed factor operation");
    changed_factor.kind = OperationKind::IntegerConstant {
        value: IntegerValue::Unsigned(3),
    };
    assert!(matches!(
        terminal_verifier::verify_module(
            &changed_affine_cast_factor,
            &decode_proof_bundle(&proof).expect("decode unchanged shared proof"),
            &AdmissionProfile::default(),
        ),
        Err(terminal_verifier::VerificationError::RejectedEvidence {
            obligation,
            ..
        }) if affine_cast_obligations.contains(&obligation)
    ));
    let mut changed_cast_then_affine_factor =
        decode_module(&semantics).expect("decode shared semantics");
    let changed_factor = changed_cast_then_affine_factor
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            operation.result.scalar_ref().map(|result| result.id) == Some(cast_then_affine_factor)
        })
        .expect("post-cast affine chain retains its landed factor operation");
    changed_factor.kind = OperationKind::IntegerConstant {
        value: IntegerValue::Unsigned(3),
    };
    assert!(matches!(
        terminal_verifier::verify_module(
            &changed_cast_then_affine_factor,
            &decode_proof_bundle(&proof).expect("decode unchanged shared proof"),
            &AdmissionProfile::default(),
        ),
        Err(terminal_verifier::VerificationError::RejectedEvidence {
            obligation,
            ..
        }) if cast_then_affine_obligations.contains(&obligation)
    ));
    let mut changed_middle_shift_count =
        decode_module(&semantics).expect("decode shared semantics");
    let changed_shift_count = changed_middle_shift_count
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            operation.result.scalar_ref().map(|result| result.id) == Some(middle_shift_count)
        })
        .expect("middle exact-shift-right landed count operation");
    changed_shift_count.kind = OperationKind::IntegerConstant {
        value: IntegerValue::Unsigned(8),
    };
    assert!(matches!(
        terminal_verifier::verify_module(
            &changed_middle_shift_count,
            &decode_proof_bundle(&proof).expect("decode unchanged shared proof"),
            &AdmissionProfile::default(),
        ),
        Err(terminal_verifier::VerificationError::RejectedEvidence {
            obligation,
            ..
        }) if obligation == nested_shift_right_obligations[1]
    ));
    let mut changed_middle_shift_left_count =
        decode_module(&semantics).expect("decode shared semantics");
    let changed_shift_left_count = changed_middle_shift_left_count
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            operation.result.scalar_ref().map(|result| result.id) == Some(middle_shift_left_count)
        })
        .expect("middle exact-shift-left landed count operation");
    changed_shift_left_count.kind = OperationKind::IntegerConstant {
        value: IntegerValue::Unsigned(3),
    };
    assert!(matches!(
        terminal_verifier::verify_module(
            &changed_middle_shift_left_count,
            &decode_proof_bundle(&proof).expect("decode unchanged shared proof"),
            &AdmissionProfile::default(),
        ),
        Err(terminal_verifier::VerificationError::RejectedEvidence {
            obligation,
            ..
        }) if obligation == nested_shift_left_obligations[1]
    ));
    let mut changed_cast_then_shift_left_count =
        decode_module(&semantics).expect("decode shared semantics");
    let changed_shift_left_count = changed_cast_then_shift_left_count
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            operation.result.scalar_ref().map(|result| result.id)
                == Some(cast_then_shift_left_middle_count)
        })
        .expect("post-cast shift-left landed middle count operation");
    changed_shift_left_count.kind = OperationKind::IntegerConstant {
        value: IntegerValue::Unsigned(3),
    };
    assert!(matches!(
        terminal_verifier::verify_module(
            &changed_cast_then_shift_left_count,
            &decode_proof_bundle(&proof).expect("decode unchanged shared proof"),
            &AdmissionProfile::default(),
        ),
        Err(terminal_verifier::VerificationError::RejectedEvidence {
            obligation,
            ..
        }) if cast_then_shift_left_obligations.contains(&obligation)
    ));
    let mut changed_shift_right_then_cast_count =
        decode_module(&semantics).expect("decode shared semantics");
    let changed_shift_right_count = changed_shift_right_then_cast_count
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            operation.result.scalar_ref().map(|result| result.id)
                == Some(shift_right_then_cast_middle_count)
        })
        .expect("pre-cast shift-right middle landed count operation");
    changed_shift_right_count.kind = OperationKind::IntegerConstant {
        value: IntegerValue::Unsigned(8),
    };
    assert!(matches!(
        terminal_verifier::verify_module(
            &changed_shift_right_then_cast_count,
            &decode_proof_bundle(&proof).expect("decode unchanged shared proof"),
            &AdmissionProfile::default(),
        ),
        Err(terminal_verifier::VerificationError::RejectedEvidence {
            obligation,
            ..
        }) if shift_right_then_cast_obligations.contains(&obligation)
    ));
    let mut changed_cast_then_shift_right_count =
        decode_module(&semantics).expect("decode shared semantics");
    let changed_shift_right_count = changed_cast_then_shift_right_count
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            operation.result.scalar_ref().map(|result| result.id)
                == Some(cast_then_shift_right_middle_count)
        })
        .expect("post-cast shift-right middle landed count operation");
    changed_shift_right_count.kind = OperationKind::IntegerConstant {
        value: IntegerValue::Unsigned(8),
    };
    assert!(matches!(
        terminal_verifier::verify_module(
            &changed_cast_then_shift_right_count,
            &decode_proof_bundle(&proof).expect("decode unchanged shared proof"),
            &AdmissionProfile::default(),
        ),
        Err(terminal_verifier::VerificationError::RejectedEvidence {
            obligation,
            ..
        }) if cast_then_shift_right_obligations.contains(&obligation)
    ));
    let mut changed_shift_left_then_cast_count =
        decode_module(&semantics).expect("decode shared semantics");
    let changed_shift_left_count = changed_shift_left_then_cast_count
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            operation.result.scalar_ref().map(|result| result.id)
                == Some(shift_left_then_cast_middle_count)
        })
        .expect("pre-cast shift-left middle landed count operation");
    changed_shift_left_count.kind = OperationKind::IntegerConstant {
        value: IntegerValue::Unsigned(3),
    };
    assert!(matches!(
        terminal_verifier::verify_module(
            &changed_shift_left_then_cast_count,
            &decode_proof_bundle(&proof).expect("decode unchanged shared proof"),
            &AdmissionProfile::default(),
        ),
        Err(terminal_verifier::VerificationError::RejectedEvidence {
            obligation,
            ..
        }) if shift_left_then_cast_obligations.contains(&obligation)
    ));
    let mut changed_nested_widen_subtract_bound =
        decode_module(&semantics).expect("decode shared semantics");
    let changed_entry = changed_nested_widen_subtract_bound.entry;
    let entry_contract = &mut changed_nested_widen_subtract_bound
        .machines
        .iter_mut()
        .find(|machine| machine.id == changed_entry)
        .expect("changed shared entry")
        .contract;
    let nested_widen_subtract_requirement = entry_contract
        .requires
        .iter()
        .position(|requirement| requirement == &widen_exact_subtract_requirement)
        .expect("shared convergence retains the nested widened exact-subtract bound");
    entry_contract.requires[nested_widen_subtract_requirement] = Proposition::LessOrEqual(
        unsigned_term(8, 4),
        ScalarTerm::value(entry.parameters[1].id, entry.parameters[1].scalar_type),
    );
    assert!(matches!(
        terminal_verifier::verify_module(
            &changed_nested_widen_subtract_bound,
            &decode_proof_bundle(&proof).expect("decode unchanged shared proof"),
            &AdmissionProfile::default(),
        ),
        Err(terminal_verifier::VerificationError::RejectedEvidence {
            obligation,
            ..
        }) if obligation == widen_exact_subtract_obligation
            || obligation == nested_subtract_obligations[2]
    ));
    for obligation in &runtime_signed_add_obligations {
        let mut missing_runtime_signed_add_proof =
            decode_proof_bundle(&proof).expect("decode shared proof");
        missing_runtime_signed_add_proof
            .evidence
            .retain(|evidence| evidence.obligation != *obligation);
        assert!(matches!(
            terminal_verifier::verify_module(
                &decode_module(&semantics).expect("decode shared semantics"),
                &missing_runtime_signed_add_proof,
                &AdmissionProfile::default(),
            ),
            Err(terminal_verifier::VerificationError::MissingEvidence(missing))
                if missing == *obligation
        ));
    }
    let changed_positive_add_requirement = Proposition::LessOrEqual(
        ScalarTerm::value(entry.parameters[5].id, entry.parameters[5].scalar_type),
        ScalarTerm::exact_integer_subtract(
            signed_arithmetic_type,
            ScalarTerm::integer(signed_arithmetic_type, IntegerValue::Signed(126)).unwrap(),
            ScalarTerm::value(entry.parameters[11].id, entry.parameters[11].scalar_type),
        )
        .unwrap(),
    );
    let changed_negative_add_requirement = Proposition::LessOrEqual(
        ScalarTerm::exact_integer_subtract(
            signed_arithmetic_type,
            ScalarTerm::integer(signed_arithmetic_type, IntegerValue::Signed(-127)).unwrap(),
            ScalarTerm::value(entry.parameters[12].id, entry.parameters[12].scalar_type),
        )
        .unwrap(),
        ScalarTerm::value(entry.parameters[5].id, entry.parameters[5].scalar_type),
    );
    for (original, replacement, obligation) in [
        (
            &runtime_positive_add_requirement,
            changed_positive_add_requirement,
            runtime_signed_add_obligations[0],
        ),
        (
            &runtime_negative_add_requirement,
            changed_negative_add_requirement,
            runtime_signed_add_obligations[1],
        ),
    ] {
        let mut changed_runtime_signed_add_bound =
            decode_module(&semantics).expect("decode shared semantics");
        let changed_entry = changed_runtime_signed_add_bound.entry;
        let entry_contract = &mut changed_runtime_signed_add_bound
            .machines
            .iter_mut()
            .find(|machine| machine.id == changed_entry)
            .expect("changed shared entry")
            .contract;
        let position = entry_contract
            .requires
            .iter()
            .position(|requirement| requirement == original)
            .expect("shared convergence retains each signed computed runtime-add bound");
        entry_contract.requires[position] = replacement;
        assert!(matches!(
            terminal_verifier::verify_module(
                &changed_runtime_signed_add_bound,
                &decode_proof_bundle(&proof).expect("decode unchanged shared proof"),
                &AdmissionProfile::default(),
            ),
            Err(terminal_verifier::VerificationError::RejectedEvidence {
                obligation: rejected,
                ..
            }) if rejected == obligation
        ));
    }
    for obligation in &runtime_signed_subtract_obligations {
        let mut missing_runtime_signed_subtract_proof =
            decode_proof_bundle(&proof).expect("decode shared proof");
        missing_runtime_signed_subtract_proof
            .evidence
            .retain(|evidence| evidence.obligation != *obligation);
        assert!(matches!(
            terminal_verifier::verify_module(
                &decode_module(&semantics).expect("decode shared semantics"),
                &missing_runtime_signed_subtract_proof,
                &AdmissionProfile::default(),
            ),
            Err(terminal_verifier::VerificationError::MissingEvidence(missing))
                if missing == *obligation
        ));
    }
    let changed_positive_subtract_requirement = Proposition::LessOrEqual(
        ScalarTerm::exact_integer_add(
            signed_arithmetic_type,
            ScalarTerm::integer(signed_arithmetic_type, IntegerValue::Signed(-127)).unwrap(),
            ScalarTerm::value(entry.parameters[13].id, entry.parameters[13].scalar_type),
        )
        .unwrap(),
        ScalarTerm::value(entry.parameters[5].id, entry.parameters[5].scalar_type),
    );
    let changed_negative_subtract_requirement = Proposition::LessOrEqual(
        ScalarTerm::value(entry.parameters[5].id, entry.parameters[5].scalar_type),
        ScalarTerm::exact_integer_add(
            signed_arithmetic_type,
            ScalarTerm::integer(signed_arithmetic_type, IntegerValue::Signed(126)).unwrap(),
            ScalarTerm::value(entry.parameters[14].id, entry.parameters[14].scalar_type),
        )
        .unwrap(),
    );
    for (original, replacement, obligation) in [
        (
            &runtime_positive_subtract_requirement,
            changed_positive_subtract_requirement,
            runtime_signed_subtract_obligations[0],
        ),
        (
            &runtime_negative_subtract_requirement,
            changed_negative_subtract_requirement,
            runtime_signed_subtract_obligations[1],
        ),
    ] {
        let mut changed_runtime_signed_subtract_bound =
            decode_module(&semantics).expect("decode shared semantics");
        let changed_entry = changed_runtime_signed_subtract_bound.entry;
        let entry_contract = &mut changed_runtime_signed_subtract_bound
            .machines
            .iter_mut()
            .find(|machine| machine.id == changed_entry)
            .expect("changed shared entry")
            .contract;
        let position = entry_contract
            .requires
            .iter()
            .position(|requirement| requirement == original)
            .expect("shared convergence retains each signed computed runtime-subtract bound");
        entry_contract.requires[position] = replacement;
        assert!(matches!(
            terminal_verifier::verify_module(
                &changed_runtime_signed_subtract_bound,
                &decode_proof_bundle(&proof).expect("decode unchanged shared proof"),
                &AdmissionProfile::default(),
            ),
            Err(terminal_verifier::VerificationError::RejectedEvidence {
                obligation: rejected,
                ..
            }) if rejected == obligation
        ));
    }
    let mut missing_shift_proof = decode_proof_bundle(&proof).expect("decode shared proof");
    missing_shift_proof
        .evidence
        .retain(|evidence| evidence.obligation != exact_shift_obligation);
    assert!(matches!(
        terminal_verifier::verify_module(
            &decode_module(&semantics).expect("decode shared semantics"),
            &missing_shift_proof,
            &AdmissionProfile::default(),
        ),
        Err(terminal_verifier::VerificationError::MissingEvidence(obligation))
            if obligation == exact_shift_obligation
    ));
    let mut changed_shift_bound = decode_module(&semantics).expect("decode shared semantics");
    let changed_entry = changed_shift_bound.entry;
    let entry_contract = &mut changed_shift_bound
        .machines
        .iter_mut()
        .find(|machine| machine.id == changed_entry)
        .expect("changed shared entry")
        .contract;
    let shift_requirement = entry_contract
        .requires
        .iter()
        .position(|requirement| requirement == &shift_upper_requirement)
        .expect("shared convergence retains the exact-shift count premise");
    entry_contract.requires[shift_requirement] = Proposition::LessOrEqual(
        ScalarTerm::value(entry.parameters[1].id, entry.parameters[1].scalar_type),
        ScalarTerm::integer(
            IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
            IntegerValue::Unsigned(6),
        )
        .unwrap(),
    );
    assert!(matches!(
        terminal_verifier::verify_module(
            &changed_shift_bound,
            &decode_proof_bundle(&proof).expect("decode unchanged shared proof"),
            &AdmissionProfile::default(),
        ),
        Err(terminal_verifier::VerificationError::RejectedEvidence {
            obligation,
            ..
        }) if obligation == exact_shift_obligation
    ));
    let mut missing_signed_count_shift_proof =
        decode_proof_bundle(&proof).expect("decode shared proof");
    missing_signed_count_shift_proof
        .evidence
        .retain(|evidence| evidence.obligation != signed_count_exact_shift_obligation);
    assert!(matches!(
        terminal_verifier::verify_module(
            &decode_module(&semantics).expect("decode shared semantics"),
            &missing_signed_count_shift_proof,
            &AdmissionProfile::default(),
        ),
        Err(terminal_verifier::VerificationError::MissingEvidence(obligation))
            if obligation == signed_count_exact_shift_obligation
    ));
    let mut changed_signed_count_shift_bound =
        decode_module(&semantics).expect("decode shared semantics");
    let changed_entry = changed_signed_count_shift_bound.entry;
    let entry_contract = &mut changed_signed_count_shift_bound
        .machines
        .iter_mut()
        .find(|machine| machine.id == changed_entry)
        .expect("changed shared entry")
        .contract;
    let signed_shift_requirement = entry_contract
        .requires
        .iter()
        .position(|requirement| requirement == &signed_shift_count_upper_requirement)
        .expect("shared convergence retains the signed exact-shift upper premise");
    entry_contract.requires[signed_shift_requirement] = Proposition::LessOrEqual(
        ScalarTerm::value(entry.parameters[6].id, entry.parameters[6].scalar_type),
        ScalarTerm::integer(
            IntegerType::new(IntegerSign::Signed, 8).unwrap(),
            IntegerValue::Signed(6),
        )
        .unwrap(),
    );
    assert!(matches!(
        terminal_verifier::verify_module(
            &changed_signed_count_shift_bound,
            &decode_proof_bundle(&proof).expect("decode unchanged shared proof"),
            &AdmissionProfile::default(),
        ),
        Err(terminal_verifier::VerificationError::RejectedEvidence {
            obligation,
            ..
        }) if obligation == signed_count_exact_shift_obligation
    ));
    let mut missing_shift_left_proof = decode_proof_bundle(&proof).expect("decode shared proof");
    missing_shift_left_proof
        .evidence
        .retain(|evidence| evidence.obligation != exact_shift_left_obligation);
    assert!(matches!(
        terminal_verifier::verify_module(
            &decode_module(&semantics).expect("decode shared semantics"),
            &missing_shift_left_proof,
            &AdmissionProfile::default(),
        ),
        Err(terminal_verifier::VerificationError::MissingEvidence(obligation))
            if obligation == exact_shift_left_obligation
    ));
    let mut missing_runtime_shift_left_proof =
        decode_proof_bundle(&proof).expect("decode shared proof");
    missing_runtime_shift_left_proof
        .evidence
        .retain(|evidence| evidence.obligation != runtime_exact_shift_left_obligation);
    assert!(matches!(
        terminal_verifier::verify_module(
            &decode_module(&semantics).expect("decode shared semantics"),
            &missing_runtime_shift_left_proof,
            &AdmissionProfile::default(),
        ),
        Err(terminal_verifier::VerificationError::MissingEvidence(obligation))
            if obligation == runtime_exact_shift_left_obligation
    ));
    let mut changed_left_shift_count = decode_module(&semantics).expect("decode shared semantics");
    let changed_entry = changed_left_shift_count.entry;
    let entry_contract = &mut changed_left_shift_count
        .machines
        .iter_mut()
        .find(|machine| machine.id == changed_entry)
        .expect("changed shared entry")
        .contract;
    let left_shift_count = entry_contract
        .requires
        .iter()
        .position(|requirement| requirement == &left_shift_count_requirement)
        .expect("shared convergence retains the runtime-left-shift count premise");
    entry_contract.requires[left_shift_count] = Proposition::LessOrEqual(
        ScalarTerm::value(entry.parameters[3].id, entry.parameters[3].scalar_type),
        ScalarTerm::integer(
            IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
            IntegerValue::Unsigned(1),
        )
        .unwrap(),
    );
    assert!(matches!(
        terminal_verifier::verify_module(
            &changed_left_shift_count,
            &decode_proof_bundle(&proof).expect("decode unchanged shared proof"),
            &AdmissionProfile::default(),
        ),
        Err(terminal_verifier::VerificationError::RejectedEvidence {
            obligation,
            ..
        }) if obligation == runtime_exact_shift_left_obligation
    ));
    let mut missing_runtime_signed_count_shift_left_proof =
        decode_proof_bundle(&proof).expect("decode shared proof");
    missing_runtime_signed_count_shift_left_proof
        .evidence
        .retain(|evidence| evidence.obligation != runtime_signed_count_shift_left_obligation);
    assert!(matches!(
        terminal_verifier::verify_module(
            &decode_module(&semantics).expect("decode shared semantics"),
            &missing_runtime_signed_count_shift_left_proof,
            &AdmissionProfile::default(),
        ),
        Err(terminal_verifier::VerificationError::MissingEvidence(obligation))
            if obligation == runtime_signed_count_shift_left_obligation
    ));
    let mut changed_signed_left_shift_count =
        decode_module(&semantics).expect("decode shared semantics");
    let changed_entry = changed_signed_left_shift_count.entry;
    let entry_contract = &mut changed_signed_left_shift_count
        .machines
        .iter_mut()
        .find(|machine| machine.id == changed_entry)
        .expect("changed shared entry")
        .contract;
    let signed_left_shift_count = entry_contract
        .requires
        .iter()
        .position(|requirement| requirement == &runtime_signed_shift_count_upper_requirement)
        .expect("shared convergence retains the signed runtime-left-shift count premise");
    entry_contract.requires[signed_left_shift_count] = Proposition::LessOrEqual(
        ScalarTerm::value(entry.parameters[15].id, entry.parameters[15].scalar_type),
        ScalarTerm::integer(
            IntegerType::new(IntegerSign::Signed, 8).unwrap(),
            IntegerValue::Signed(1),
        )
        .unwrap(),
    );
    assert!(matches!(
        terminal_verifier::verify_module(
            &changed_signed_left_shift_count,
            &decode_proof_bundle(&proof).expect("decode unchanged shared proof"),
            &AdmissionProfile::default(),
        ),
        Err(terminal_verifier::VerificationError::RejectedEvidence {
            obligation,
            ..
        }) if obligation == runtime_signed_count_shift_left_obligation
    ));
    for obligation in &signed_value_shift_left_obligations {
        let mut missing_signed_value_shift_left_proof =
            decode_proof_bundle(&proof).expect("decode shared proof");
        missing_signed_value_shift_left_proof
            .evidence
            .retain(|evidence| evidence.obligation != *obligation);
        assert!(matches!(
            terminal_verifier::verify_module(
                &decode_module(&semantics).expect("decode shared semantics"),
                &missing_signed_value_shift_left_proof,
                &AdmissionProfile::default(),
            ),
            Err(terminal_verifier::VerificationError::MissingEvidence(missing))
                if missing == *obligation
        ));
    }
    for (original, replacement) in [
        (
            &signed_shift_value_lower_requirement,
            Proposition::LessOrEqual(
                ScalarTerm::integer(
                    signed_arithmetic_type,
                    signed_arithmetic_type.minimum_value(),
                )
                .unwrap(),
                ScalarTerm::value(entry.parameters[5].id, entry.parameters[5].scalar_type),
            ),
        ),
        (
            &signed_shift_value_upper_requirement,
            Proposition::LessOrEqual(
                ScalarTerm::value(entry.parameters[5].id, entry.parameters[5].scalar_type),
                ScalarTerm::integer(
                    signed_arithmetic_type,
                    signed_arithmetic_type.maximum_value(),
                )
                .unwrap(),
            ),
        ),
    ] {
        let mut changed_signed_value_shift_bound =
            decode_module(&semantics).expect("decode shared semantics");
        let changed_entry = changed_signed_value_shift_bound.entry;
        let entry_contract = &mut changed_signed_value_shift_bound
            .machines
            .iter_mut()
            .find(|machine| machine.id == changed_entry)
            .expect("changed shared entry")
            .contract;
        let position = entry_contract
            .requires
            .iter()
            .position(|requirement| requirement == original)
            .expect("shared convergence retains each signed-value shift bound");
        entry_contract.requires[position] = replacement;
        assert!(matches!(
            terminal_verifier::verify_module(
                &changed_signed_value_shift_bound,
                &decode_proof_bundle(&proof).expect("decode unchanged shared proof"),
                &AdmissionProfile::default(),
            ),
            Err(terminal_verifier::VerificationError::RejectedEvidence {
                obligation,
                ..
            }) if obligation == signed_value_shift_left_obligations[0]
        ));
    }
    }

    let [token] = entry.structural_parameters.as_slice() else {
        panic!("shared integer convergence retains its cleanup root")
    };

    let structural_arguments = [TerminalStructuralValue {
        opaque_identity: token.place.get(),
        structural_type: token.structural_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    }];
    for (
        input,
        small,
        divisor,
        count,
        signed,
        signed_arithmetic,
        signed_divisor,
        negative_divisor,
        bounded_negative_divisor,
        add_left,
        add_right,
        positive_addend,
        negative_addend,
        positive_subtrahend,
        negative_subtrahend,
        signed_count,
        enabled,
        wide,
    ) in [
        (
            3_u128, 4_u128, 2_u128, 1_u128, -1_i128, 2_i128, 2_i128, -2_i128, -1_i128, 200_u128,
            55_u128, 3_i128, -3_i128, 3_i128, -3_i128, 1_i128, false, 512_u128,
        ),
        (
            3, 4, 2, 1, -1, 2, 1, -3, -2, 100, 100, 1, -1, 1, -1, 1, true, 512,
        ),
        (
            3, 5, 3, 2, 3, 3, 2, -4, -1, 254, 1, 2, -2, 2, -2, 2, true, 512,
        ),
        (
            4, 4, 2, 2, 4, 2, 3, -2, -3, 0, 255, 4, -4, 4, -4, 2, true, 512,
        ),
        (
            10, 4, 4, 1, -2, 0, 4, -5, -1, 42, 7, 5, -5, 5, -5, 1, true, 512,
        ),
    ] {
        let mask = u128::from(u64::MAX);
        let bitwise_not = (!input) & mask;
        let wrapped_add = (input + 1) & mask;
        let nested_wrapped_add = (wrapped_add + 1) & mask;
        let mut handler = AcceptTerminalEffects;
        let measured = interpret_terminal_artifact_with_effect_handler_measured(
            &semantics,
            &proof,
            &AdmissionProfile::default(),
            &[
                TerminalScalarValue::Integer {
                    scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                    value: IntegerValue::Unsigned(input),
                },
                TerminalScalarValue::Integer {
                    scalar_type: IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
                    value: IntegerValue::Unsigned(small),
                },
                TerminalScalarValue::Integer {
                    scalar_type: IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
                    value: IntegerValue::Unsigned(divisor),
                },
                TerminalScalarValue::Integer {
                    scalar_type: IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
                    value: IntegerValue::Unsigned(count),
                },
                TerminalScalarValue::Integer {
                    scalar_type: IntegerType::new(IntegerSign::Signed, 64).unwrap(),
                    value: IntegerValue::Signed(signed),
                },
                TerminalScalarValue::Integer {
                    scalar_type: IntegerType::new(IntegerSign::Signed, 8).unwrap(),
                    value: IntegerValue::Signed(signed_arithmetic),
                },
                TerminalScalarValue::Integer {
                    scalar_type: IntegerType::new(IntegerSign::Signed, 8).unwrap(),
                    value: IntegerValue::Signed(signed_divisor),
                },
                TerminalScalarValue::Integer {
                    scalar_type: IntegerType::new(IntegerSign::Signed, 8).unwrap(),
                    value: IntegerValue::Signed(negative_divisor),
                },
                TerminalScalarValue::Integer {
                    scalar_type: IntegerType::new(IntegerSign::Signed, 8).unwrap(),
                    value: IntegerValue::Signed(bounded_negative_divisor),
                },
                TerminalScalarValue::Integer {
                    scalar_type: IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
                    value: IntegerValue::Unsigned(add_left),
                },
                TerminalScalarValue::Integer {
                    scalar_type: IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
                    value: IntegerValue::Unsigned(add_right),
                },
                TerminalScalarValue::Integer {
                    scalar_type: IntegerType::new(IntegerSign::Signed, 8).unwrap(),
                    value: IntegerValue::Signed(positive_addend),
                },
                TerminalScalarValue::Integer {
                    scalar_type: IntegerType::new(IntegerSign::Signed, 8).unwrap(),
                    value: IntegerValue::Signed(negative_addend),
                },
                TerminalScalarValue::Integer {
                    scalar_type: IntegerType::new(IntegerSign::Signed, 8).unwrap(),
                    value: IntegerValue::Signed(positive_subtrahend),
                },
                TerminalScalarValue::Integer {
                    scalar_type: IntegerType::new(IntegerSign::Signed, 8).unwrap(),
                    value: IntegerValue::Signed(negative_subtrahend),
                },
                TerminalScalarValue::Integer {
                    scalar_type: IntegerType::new(IntegerSign::Signed, 8).unwrap(),
                    value: IntegerValue::Signed(signed_count),
                },
                TerminalScalarValue::Boolean(enabled),
                TerminalScalarValue::Integer {
                    scalar_type: IntegerType::new(IntegerSign::Unsigned, 16).unwrap(),
                    value: IntegerValue::Unsigned(wide),
                },
            ],
            &structural_arguments,
            &mut handler,
        )
        .expect("shared integer convergence interprets");
        assert_eq!(
            measured.value(),
            TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(
                ((wrapped_add < 4) || (bitwise_not < 1) || (input <= 9))
                    && nested_wrapped_add < 5
                    && small < 5
                    && input < 5
                    && input < 256
                    && small < 6
                    && small < 7
                    && small + 1 < 6
                    && small + 1 + 1 + 1 < 8
                    && (!(small + 3) & u128::from(u8::MAX)) < 255
                    && small - 3 < 255
                    && small - 1 - 1 - 1 < 5
                    && (!(small + 3) & u128::from(u16::MAX)) < 65535
                    && ((small + 1) & (small * 2)) < 255
                    && 127 - small < 125
                    && small - divisor < 4
                    && small * 2 < 10
                    && ((small * 2) * 3) < 255
                    && small * divisor < 50
                    && small / 2 < 3
                    && small % 2 <= 1
                    && ((small / 2) % 3) / 2 < 2
                    && wide / 256 < 255
                    && (wide / 2) % 3 < 3
                    && signed % -3 < 3
                    && wide % 3 < 3
                    && (small / divisor) % 2 < 2
                    && (input / divisor) % 2 < 2
                    && (signed_arithmetic / signed_divisor) % -3 < 3
                    && (signed_arithmetic / negative_divisor) % 3 < 3
                    && (signed / signed_divisor) % -3 < 3
                    && (signed / negative_divisor) % 3 < 3
                    && small / divisor < 6
                    && small % divisor <= small
                    && (small >> small) < 1
                    && (signed_arithmetic >> signed_divisor) < 4
                    && ((small >> 1) >> 2) < 2
                    && ((small << 1) << 2) < 255
                    && (small << 1) < 11
                    && (small << count) < 29
                    && (small << signed_count) < 255
                    && (signed_arithmetic << 2) < 127
                    && (signed_arithmetic << count) < 127
                    && (signed_arithmetic << signed_count) < 127
                    && signed < 4
                    && small < 4
                    && signed_arithmetic < 4
                    && signed_arithmetic + 1 < 4
                    && signed_arithmetic - 1 < 4
                    && signed_arithmetic - 1 < 4
                    && signed_arithmetic + 1 < 4
                    && ((small + 3) - 2) + 1 < 255
                    && ((signed_arithmetic + 3) - 5) + 1 < 127
                    && signed_arithmetic * 3 < 4
                    && signed_arithmetic * -3 < 4
                    && signed_arithmetic * signed_divisor <= 127
                    && signed_arithmetic * negative_divisor <= 127
                    && signed_arithmetic / 2 < 4
                    && signed_arithmetic % -2 <= 1
                    && signed_arithmetic / signed_divisor < 4
                    && signed_arithmetic % signed_divisor <= signed_arithmetic
                    && signed_arithmetic / negative_divisor < 4
                    && signed_arithmetic % negative_divisor <= signed_arithmetic
                    && signed_arithmetic / bounded_negative_divisor < 4
                    && signed_arithmetic % bounded_negative_divisor <= signed_arithmetic
                    && add_left + add_right <= 255
                    && signed_arithmetic + positive_addend <= 127
                    && signed_arithmetic + negative_addend < 4
                    && signed_arithmetic - positive_subtrahend < 4
                    && signed_arithmetic - negative_subtrahend <= 127
                    && input == 3
                    && enabled
            ))
        );
    }
}
