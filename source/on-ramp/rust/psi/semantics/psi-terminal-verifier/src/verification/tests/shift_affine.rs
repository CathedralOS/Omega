use super::super::*;
use psi_core::{IntegerType, ScalarType, ValueId};

#[test]
fn exact_affine_shift_cast_sandwich_reconstructs_both_directions_and_zero_locally() {
    let source_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16 source");
    let target_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 target");
    let i8_count = IntegerType::new(IntegerSign::Signed, 8).expect("i8 count");
    let u16_count = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16 count");

    let affine_root_id = ValueId::new(1401).expect("affine root");
    let affine_root = ScalarTerm::value(affine_root_id, ScalarType::Integer(source_type));
    let source_add = ScalarTerm::value(
        ValueId::new(1402).expect("source add"),
        ScalarType::Integer(source_type),
    );
    let source_multiply = ScalarTerm::value(
        ValueId::new(1403).expect("source multiply"),
        ScalarType::Integer(source_type),
    );
    let affine_cast = ScalarTerm::value(
        ValueId::new(1404).expect("affine cast"),
        ScalarType::Integer(target_type),
    );
    let target_right = ScalarTerm::value(
        ValueId::new(1405).expect("target right"),
        ScalarType::Integer(target_type),
    );
    let affine_to_shift_definitions = vec![
        Proposition::Equal(
            source_add.clone(),
            ScalarTerm::exact_integer_add(
                source_type,
                affine_root.clone(),
                ScalarTerm::integer(source_type, IntegerValue::Unsigned(1)).expect("1u16"),
            )
            .expect("root + 1"),
        ),
        Proposition::Equal(
            source_multiply.clone(),
            ScalarTerm::exact_integer_multiply(
                source_type,
                source_add,
                ScalarTerm::integer(source_type, IntegerValue::Unsigned(2)).expect("2u16"),
            )
            .expect("(root + 1) * 2"),
        ),
        Proposition::Equal(
            affine_cast.clone(),
            ScalarTerm::integer_exact_cast(source_type, target_type, source_multiply)
                .expect("u16 to u8 cast"),
        ),
        Proposition::Equal(
            target_right.clone(),
            ScalarTerm::exact_integer_shift_right(
                target_type,
                i8_count,
                affine_cast.clone(),
                ScalarTerm::integer(i8_count, IntegerValue::Signed(1)).expect("1i8"),
            )
            .expect("cast >> 1"),
        ),
    ];
    assert_eq!(
        exact_integer_affine_cast_shift_obligation(
            target_type,
            target_right.clone(),
            2,
            &affine_to_shift_definitions,
            affine_to_shift_definitions.len(),
            &BTreeSet::from([affine_root_id]),
        ),
        Some(Proposition::LessOrEqual(
            affine_root.clone(),
            ScalarTerm::integer(source_type, IntegerValue::Unsigned(62)).expect("62u16"),
        )),
    );
    assert_eq!(
        exact_integer_affine_cast_shift_obligation(
            target_type,
            target_right,
            2,
            &[Proposition::Truth],
            1,
            &BTreeSet::from([affine_root_id]),
        ),
        None,
        "stale target definitions cannot authorize the affine-to-shift direction",
    );

    let zero = ScalarTerm::value(
        ValueId::new(1406).expect("zero"),
        ScalarType::Integer(source_type),
    );
    let constant = ScalarTerm::value(
        ValueId::new(1407).expect("constant"),
        ScalarType::Integer(source_type),
    );
    let constant_cast = ScalarTerm::value(
        ValueId::new(1408).expect("constant cast"),
        ScalarType::Integer(target_type),
    );
    let constant_definitions = vec![
        Proposition::Equal(
            zero.clone(),
            ScalarTerm::exact_integer_multiply(
                source_type,
                affine_root.clone(),
                ScalarTerm::integer(source_type, IntegerValue::Unsigned(0)).expect("0u16"),
            )
            .expect("root * 0"),
        ),
        Proposition::Equal(
            constant.clone(),
            ScalarTerm::exact_integer_add(
                source_type,
                zero,
                ScalarTerm::integer(source_type, IntegerValue::Unsigned(255)).expect("255u16"),
            )
            .expect("zero + 255"),
        ),
        Proposition::Equal(
            constant_cast.clone(),
            ScalarTerm::integer_exact_cast(source_type, target_type, constant)
                .expect("constant cast"),
        ),
    ];
    assert_eq!(
        exact_integer_affine_cast_shift_obligation(
            target_type,
            constant_cast,
            2,
            &constant_definitions,
            constant_definitions.len(),
            &BTreeSet::from([affine_root_id]),
        ),
        Some(Proposition::Falsehood),
        "a constant source affine value outside the target-left interval is mathematically false",
    );

    let shift_root_id = ValueId::new(1411).expect("shift root");
    let shift_root = ScalarTerm::value(shift_root_id, ScalarType::Integer(source_type));
    let source_right = ScalarTerm::value(
        ValueId::new(1412).expect("source right"),
        ScalarType::Integer(source_type),
    );
    let source_left = ScalarTerm::value(
        ValueId::new(1413).expect("source left"),
        ScalarType::Integer(source_type),
    );
    let shift_cast = ScalarTerm::value(
        ValueId::new(1414).expect("shift cast"),
        ScalarType::Integer(target_type),
    );
    let target_add = ScalarTerm::value(
        ValueId::new(1415).expect("target add"),
        ScalarType::Integer(target_type),
    );
    let shift_to_affine_definitions = vec![
        Proposition::Equal(
            source_right.clone(),
            ScalarTerm::exact_integer_shift_right(
                source_type,
                i8_count,
                shift_root.clone(),
                ScalarTerm::integer(i8_count, IntegerValue::Signed(1)).expect("1i8"),
            )
            .expect("root >> 1"),
        ),
        Proposition::Equal(
            source_left.clone(),
            ScalarTerm::exact_integer_shift_left(
                source_type,
                u16_count,
                source_right,
                ScalarTerm::integer(u16_count, IntegerValue::Unsigned(2)).expect("2u16"),
            )
            .expect("(root >> 1) << 2"),
        ),
        Proposition::Equal(
            shift_cast.clone(),
            ScalarTerm::integer_exact_cast(source_type, target_type, source_left)
                .expect("shift cast"),
        ),
        Proposition::Equal(
            target_add.clone(),
            ScalarTerm::exact_integer_add(
                target_type,
                shift_cast.clone(),
                ScalarTerm::integer(target_type, IntegerValue::Unsigned(3)).expect("3u8"),
            )
            .expect("cast + 3"),
        ),
    ];
    assert_eq!(
        exact_integer_shift_cast_affine_obligation(
            target_type,
            target_add,
            IntegerValue::Unsigned(2),
            ExactIntegerAffineOperation::Multiply,
            &shift_to_affine_definitions,
            shift_to_affine_definitions.len(),
            &BTreeSet::from([shift_root_id]),
        ),
        Some(Proposition::LessOrEqual(
            shift_root.clone(),
            ScalarTerm::integer(source_type, IntegerValue::Unsigned(63)).expect("63u16"),
        )),
    );
    assert_eq!(
        exact_integer_shift_cast_affine_obligation(
            target_type,
            shift_cast,
            IntegerValue::Unsigned(0),
            ExactIntegerAffineOperation::Multiply,
            &shift_to_affine_definitions[..3],
            3,
            &BTreeSet::from([shift_root_id]),
        ),
        Some(Proposition::Truth),
        "a target zero coefficient decides only the current prefix after the source shift walk",
    );
}

#[test]
fn exact_arithmetic_then_shift_chain_reconstructs_affine_preimages_independently() {
    let value_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let i8_count = IntegerType::new(IntegerSign::Signed, 8).expect("i8 count");
    let u16_count = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16 count");
    let root_id = ValueId::new(341).expect("root");
    let root = ScalarTerm::value(root_id, ScalarType::Integer(value_type));
    let add = ScalarTerm::value(
        ValueId::new(342).expect("add"),
        ScalarType::Integer(value_type),
    );
    let multiply = ScalarTerm::value(
        ValueId::new(343).expect("multiply"),
        ScalarType::Integer(value_type),
    );
    let right = ScalarTerm::value(
        ValueId::new(344).expect("right"),
        ScalarType::Integer(value_type),
    );
    let definitions = vec![
        Proposition::Equal(
            add.clone(),
            ScalarTerm::exact_integer_add(
                value_type,
                root.clone(),
                ScalarTerm::integer(value_type, IntegerValue::Unsigned(3)).expect("3u8"),
            )
            .expect("root + 3"),
        ),
        Proposition::Equal(
            multiply.clone(),
            ScalarTerm::exact_integer_multiply(
                value_type,
                add,
                ScalarTerm::integer(value_type, IntegerValue::Unsigned(2)).expect("2u8"),
            )
            .expect("(root + 3) * 2"),
        ),
        Proposition::Equal(
            right.clone(),
            ScalarTerm::exact_integer_shift_right(
                value_type,
                i8_count,
                multiply,
                ScalarTerm::integer(i8_count, IntegerValue::Signed(1)).expect("1i8"),
            )
            .expect("((root + 3) * 2) >> 1"),
        ),
    ];
    assert_eq!(
        exact_integer_arithmetic_then_shift_chain_obligation(
            value_type,
            right.clone(),
            2,
            &definitions,
            definitions.len(),
            &BTreeSet::from([root_id]),
        ),
        Some(Proposition::LessOrEqual(
            root.clone(),
            ScalarTerm::integer(value_type, IntegerValue::Unsigned(60)).expect("60u8"),
        )),
    );
    assert_eq!(
        exact_integer_arithmetic_then_shift_chain_obligation(
            value_type,
            right,
            2,
            &[Proposition::Truth],
            1,
            &BTreeSet::from([root_id]),
        ),
        None,
        "stale definitions cannot authorize the computed left prefix",
    );

    let signed_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let signed_root_id = ValueId::new(345).expect("signed root");
    let signed_root = ScalarTerm::value(signed_root_id, ScalarType::Integer(signed_type));
    let signed_subtract = ScalarTerm::value(
        ValueId::new(346).expect("signed subtract"),
        ScalarType::Integer(signed_type),
    );
    let signed_multiply = ScalarTerm::value(
        ValueId::new(347).expect("signed multiply"),
        ScalarType::Integer(signed_type),
    );
    let signed_right = ScalarTerm::value(
        ValueId::new(348).expect("signed right"),
        ScalarType::Integer(signed_type),
    );
    let signed_definitions = vec![
        Proposition::Equal(
            signed_subtract.clone(),
            ScalarTerm::exact_integer_subtract(
                signed_type,
                signed_root.clone(),
                ScalarTerm::integer(signed_type, IntegerValue::Signed(-3)).expect("-3i8"),
            )
            .expect("root - -3"),
        ),
        Proposition::Equal(
            signed_multiply.clone(),
            ScalarTerm::exact_integer_multiply(
                signed_type,
                signed_subtract,
                ScalarTerm::integer(signed_type, IntegerValue::Signed(2)).expect("2i8"),
            )
            .expect("(root - -3) * 2"),
        ),
        Proposition::Equal(
            signed_right.clone(),
            ScalarTerm::exact_integer_shift_right(
                signed_type,
                u16_count,
                signed_multiply,
                ScalarTerm::integer(u16_count, IntegerValue::Unsigned(1)).expect("1u16"),
            )
            .expect("((root - -3) * 2) >> 1"),
        ),
    ];
    assert_eq!(
        exact_integer_arithmetic_then_shift_chain_obligation(
            signed_type,
            signed_right,
            2,
            &signed_definitions,
            signed_definitions.len(),
            &BTreeSet::from([signed_root_id]),
        ),
        Some(canonical_conjunction(vec![
            Proposition::LessOrEqual(
                ScalarTerm::integer(signed_type, IntegerValue::Signed(-35)).expect("-35i8"),
                signed_root.clone(),
            ),
            Proposition::LessOrEqual(
                signed_root,
                ScalarTerm::integer(signed_type, IntegerValue::Signed(28)).expect("28i8"),
            ),
        ])),
    );
}

#[test]
fn exact_arithmetic_then_shift_chain_handles_zero_and_checked_composition_failure() {
    let value_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let root_id = ValueId::new(351).expect("root");
    let root = ScalarTerm::value(root_id, ScalarType::Integer(value_type));
    let zero = ScalarTerm::value(
        ValueId::new(352).expect("zero"),
        ScalarType::Integer(value_type),
    );
    let constant = ScalarTerm::value(
        ValueId::new(353).expect("constant"),
        ScalarType::Integer(value_type),
    );
    let definitions = vec![
        Proposition::Equal(
            zero.clone(),
            ScalarTerm::exact_integer_multiply(
                value_type,
                root,
                ScalarTerm::integer(value_type, IntegerValue::Unsigned(0)).expect("0u8"),
            )
            .expect("root * 0"),
        ),
        Proposition::Equal(
            constant.clone(),
            ScalarTerm::exact_integer_add(
                value_type,
                zero,
                ScalarTerm::integer(value_type, IntegerValue::Unsigned(255)).expect("255u8"),
            )
            .expect("zero + 255"),
        ),
    ];
    assert_eq!(
        exact_integer_arithmetic_then_shift_chain_obligation(
            value_type,
            constant,
            1,
            &definitions,
            definitions.len(),
            &BTreeSet::from([root_id]),
        ),
        Some(Proposition::Falsehood),
        "a constant affine result outside the left-shift interval is mathematically false",
    );

    let wide_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
    let wide_root_id = ValueId::new(354).expect("wide root");
    let wide_root = ScalarTerm::value(wide_root_id, ScalarType::Integer(wide_type));
    let first = ScalarTerm::value(
        ValueId::new(355).expect("first"),
        ScalarType::Integer(wide_type),
    );
    let second = ScalarTerm::value(
        ValueId::new(356).expect("second"),
        ScalarType::Integer(wide_type),
    );
    let third = ScalarTerm::value(
        ValueId::new(357).expect("third"),
        ScalarType::Integer(wide_type),
    );
    let wide_definitions = vec![
        Proposition::Equal(
            first.clone(),
            ScalarTerm::exact_integer_multiply(
                wide_type,
                wide_root,
                ScalarTerm::integer(wide_type, IntegerValue::Unsigned(u64::MAX as u128))
                    .expect("u64 max"),
            )
            .expect("root * max"),
        ),
        Proposition::Equal(
            second.clone(),
            ScalarTerm::exact_integer_multiply(
                wide_type,
                first,
                ScalarTerm::integer(wide_type, IntegerValue::Unsigned(u64::MAX as u128))
                    .expect("u64 max"),
            )
            .expect("prior * max"),
        ),
        Proposition::Equal(
            third.clone(),
            ScalarTerm::exact_integer_multiply(
                wide_type,
                second,
                ScalarTerm::integer(wide_type, IntegerValue::Unsigned(2)).expect("2u64"),
            )
            .expect("prior * 2"),
        ),
    ];
    assert_eq!(
        exact_integer_arithmetic_then_shift_chain_obligation(
            wide_type,
            third,
            1,
            &wide_definitions,
            wide_definitions.len(),
            &BTreeSet::from([wide_root_id]),
        ),
        None,
        "checked affine-composition failure admits no computed-shift family",
    );
}

#[test]
fn exact_shift_then_arithmetic_chain_replays_shifts_before_each_affine_prefix() {
    let value_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let i8_count = IntegerType::new(IntegerSign::Signed, 8).expect("i8 count");
    let u16_count = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16 count");
    let root_id = ValueId::new(361).expect("root");
    let root = ScalarTerm::value(root_id, ScalarType::Integer(value_type));
    let right = ScalarTerm::value(
        ValueId::new(362).expect("right"),
        ScalarType::Integer(value_type),
    );
    let left = ScalarTerm::value(
        ValueId::new(363).expect("left"),
        ScalarType::Integer(value_type),
    );
    let add = ScalarTerm::value(
        ValueId::new(364).expect("add"),
        ScalarType::Integer(value_type),
    );
    let definitions = vec![
        Proposition::Equal(
            right.clone(),
            ScalarTerm::exact_integer_shift_right(
                value_type,
                i8_count,
                root.clone(),
                ScalarTerm::integer(i8_count, IntegerValue::Signed(1)).expect("1i8"),
            )
            .expect("root >> 1"),
        ),
        Proposition::Equal(
            left.clone(),
            ScalarTerm::exact_integer_shift_left(
                value_type,
                u16_count,
                right,
                ScalarTerm::integer(u16_count, IntegerValue::Unsigned(2)).expect("2u16"),
            )
            .expect("(root >> 1) << 2"),
        ),
        Proposition::Equal(
            add.clone(),
            ScalarTerm::exact_integer_add(
                value_type,
                left,
                ScalarTerm::integer(value_type, IntegerValue::Unsigned(3)).expect("3u8"),
            )
            .expect("shifted + 3"),
        ),
    ];
    assert_eq!(
        exact_integer_shift_then_arithmetic_chain_obligation(
            value_type,
            add.clone(),
            IntegerValue::Unsigned(2),
            ExactIntegerAffineOperation::Multiply,
            &definitions,
            definitions.len(),
            &BTreeSet::from([root_id]),
        ),
        Some(Proposition::LessOrEqual(
            root.clone(),
            ScalarTerm::integer(value_type, IntegerValue::Unsigned(63)).expect("63u8"),
        )),
    );
    assert_eq!(
        exact_integer_shift_then_arithmetic_chain_obligation(
            value_type,
            add,
            IntegerValue::Unsigned(2),
            ExactIntegerAffineOperation::Multiply,
            &[Proposition::Truth],
            1,
            &BTreeSet::from([root_id]),
        ),
        None,
        "stale definitions cannot authorize the shift-rooted affine suffix",
    );

    let shifted = ScalarTerm::value(
        ValueId::new(365).expect("shifted"),
        ScalarType::Integer(value_type),
    );
    let zero = ScalarTerm::value(
        ValueId::new(366).expect("zero"),
        ScalarType::Integer(value_type),
    );
    let zero_definitions = vec![
        Proposition::Equal(
            shifted.clone(),
            ScalarTerm::exact_integer_shift_left(
                value_type,
                i8_count,
                root,
                ScalarTerm::integer(i8_count, IntegerValue::Signed(1)).expect("1i8"),
            )
            .expect("root << 1"),
        ),
        Proposition::Equal(
            zero.clone(),
            ScalarTerm::exact_integer_multiply(
                value_type,
                shifted,
                ScalarTerm::integer(value_type, IntegerValue::Unsigned(0)).expect("0u8"),
            )
            .expect("shifted * 0"),
        ),
    ];
    assert_eq!(
        exact_integer_shift_then_arithmetic_chain_obligation(
            value_type,
            zero,
            IntegerValue::Unsigned(255),
            ExactIntegerAffineOperation::Add,
            &zero_definitions,
            zero_definitions.len(),
            &BTreeSet::from([root_id]),
        ),
        Some(Proposition::Truth),
        "A=0 decides only this outer obligation after validating the complete shift root",
    );
}

#[test]
fn exact_mixed_shift_preimage_distinguishes_empty_from_arithmetic_failure() {
    let value_type = IntegerType::new(IntegerSign::Signed, 64).expect("i64");
    let count_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 count");
    let one = ScalarTerm::integer(count_type, IntegerValue::Unsigned(1)).expect("1u8");
    let value = ScalarTerm::integer(value_type, IntegerValue::Signed(0)).expect("0i64");
    let left =
        ScalarTerm::exact_integer_shift_left(value_type, count_type, value.clone(), one.clone())
            .expect("left shape");
    assert_eq!(
        exact_integer_mixed_shift_preimage(value_type, (1, 1), &left, 1),
        Ok(None),
        "a mathematically empty inverse interval is an ordinary false proposition",
    );
    let right = ScalarTerm::exact_integer_shift_right(value_type, count_type, value, one)
        .expect("right shape");
    assert_eq!(
        exact_integer_mixed_shift_preimage(value_type, (i128::MAX, i128::MAX), &right, 1),
        Err(()),
        "checked interval arithmetic failure is not admitted as a false proposition",
    );
}
