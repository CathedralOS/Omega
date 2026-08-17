use super::super::*;
use psi_core::{IntegerType, ScalarType, ValueId};

#[test]
fn exact_divide_remainder_cross_cast_reconstructs_carrier_total_target_prefixes() {
    let source_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16 source");
    let target_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 target");
    let i8_count = IntegerType::new(IntegerSign::Signed, 8).expect("i8 count");
    let root_id = ValueId::new(1501).expect("divide root");
    let root = ScalarTerm::value(root_id, ScalarType::Integer(source_type));
    let divided = ScalarTerm::value(
        ValueId::new(1502).expect("divided"),
        ScalarType::Integer(source_type),
    );
    let remainder = ScalarTerm::value(
        ValueId::new(1503).expect("remainder"),
        ScalarType::Integer(source_type),
    );
    let cast = ScalarTerm::value(
        ValueId::new(1504).expect("cast"),
        ScalarType::Integer(target_type),
    );
    let target_add = ScalarTerm::value(
        ValueId::new(1505).expect("target add"),
        ScalarType::Integer(target_type),
    );
    let target_right = ScalarTerm::value(
        ValueId::new(1506).expect("target right"),
        ScalarType::Integer(target_type),
    );
    let definitions = vec![
        Proposition::Equal(
            divided.clone(),
            ScalarTerm::exact_integer_divide(
                source_type,
                root.clone(),
                ScalarTerm::integer(source_type, IntegerValue::Unsigned(2)).expect("2u16"),
            )
            .expect("root / 2"),
        ),
        Proposition::Equal(
            remainder.clone(),
            ScalarTerm::exact_integer_remainder(
                source_type,
                divided,
                ScalarTerm::integer(source_type, IntegerValue::Unsigned(64)).expect("64u16"),
            )
            .expect("divided % 64"),
        ),
        Proposition::Equal(
            cast.clone(),
            ScalarTerm::integer_exact_cast(source_type, target_type, remainder)
                .expect("carrier-total u16 to u8 cast"),
        ),
        Proposition::Equal(
            target_add.clone(),
            ScalarTerm::exact_integer_add(
                target_type,
                cast.clone(),
                ScalarTerm::integer(target_type, IntegerValue::Unsigned(1)).expect("1u8"),
            )
            .expect("cast + 1"),
        ),
        Proposition::Equal(
            target_right.clone(),
            ScalarTerm::exact_integer_shift_right(
                target_type,
                i8_count,
                cast.clone(),
                ScalarTerm::integer(i8_count, IntegerValue::Signed(1)).expect("1i8"),
            )
            .expect("cast >> 1"),
        ),
    ];
    let parameters = BTreeSet::from([root_id]);
    assert_eq!(
        exact_integer_divide_remainder_cast_affine_obligation(
            target_type,
            target_add,
            IntegerValue::Unsigned(2),
            ExactIntegerAffineOperation::Multiply,
            &definitions,
            4,
            &parameters,
        ),
        Some(Proposition::Truth),
        "the full [0,63] remainder hull fits the checked (value + 1) * 2 prefix",
    );
    assert_eq!(
        exact_integer_divide_remainder_cast_affine_obligation(
            target_type,
            cast.clone(),
            IntegerValue::Unsigned(64),
            ExactIntegerAffineOperation::Subtract,
            &definitions,
            3,
            &parameters,
        ),
        Some(Proposition::Falsehood),
        "a hull disjoint from the current safe interval is canonical falsehood",
    );
    assert_eq!(
        exact_integer_divide_remainder_cast_affine_obligation(
            target_type,
            cast.clone(),
            IntegerValue::Unsigned(200),
            ExactIntegerAffineOperation::Add,
            &definitions,
            3,
            &parameters,
        ),
        None,
        "partial overlap needs a guard-sensitive preimage and remains outside the family",
    );
    assert_eq!(
        exact_integer_divide_remainder_cast_affine_obligation(
            target_type,
            cast,
            IntegerValue::Unsigned(0),
            ExactIntegerAffineOperation::Multiply,
            &definitions,
            3,
            &parameters,
        ),
        Some(Proposition::Truth),
        "zero decides only the current target prefix after the complete source walk",
    );
    assert_eq!(
        exact_integer_divide_remainder_cast_shift_obligation(
            target_type,
            target_right,
            2,
            &definitions,
            definitions.len(),
            &parameters,
        ),
        Some(Proposition::Truth),
        "the target-right preimage admits the whole hull before the target-left prefix",
    );
    assert_eq!(
        exact_integer_divide_remainder_cast_shift_obligation(
            target_type,
            ScalarTerm::value(
                ValueId::new(1507).expect("stale target"),
                ScalarType::Integer(target_type),
            ),
            2,
            &definitions,
            definitions.len(),
            &parameters,
        ),
        None,
        "stale target definitions cannot authorize carrier-total replay",
    );
}

#[test]
fn exact_divide_remainder_cast_sandwich_keeps_each_obligation_independent() {
    let source_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16 source");
    let target_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8 target");
    let root_id = ValueId::new(1508).expect("sandwich root");
    let root = ScalarTerm::value(root_id, ScalarType::Integer(source_type));
    let remainder = ScalarTerm::value(
        ValueId::new(1509).expect("source remainder"),
        ScalarType::Integer(source_type),
    );
    let cast = ScalarTerm::value(
        ValueId::new(1510).expect("sandwich cast"),
        ScalarType::Integer(target_type),
    );
    let definitions = vec![
        Proposition::Equal(
            remainder.clone(),
            ScalarTerm::exact_integer_remainder(
                source_type,
                root,
                ScalarTerm::integer(source_type, IntegerValue::Unsigned(64)).expect("64u16"),
            )
            .expect("root % 64"),
        ),
        Proposition::Equal(
            cast.clone(),
            ScalarTerm::integer_exact_cast(source_type, target_type, remainder.clone())
                .expect("carrier-total u16 remainder to i8 cast"),
        ),
    ];
    assert_eq!(
        exact_integer_cast_obligation(
            source_type,
            target_type,
            remainder,
            &definitions[..1],
            &BTreeSet::from([root_id]),
        ),
        Proposition::Truth,
        "the cast replays only the complete source-chain hull",
    );
    let two = ScalarTerm::integer(target_type, IntegerValue::Signed(2)).expect("2i8");
    let three = ScalarTerm::integer(target_type, IntegerValue::Signed(3)).expect("3i8");
    assert_eq!(
        exact_integer_divide_obligation(target_type, cast.clone(), two, &definitions,),
        Proposition::Truth,
        "the target divide uses only its independently safe divisor",
    );
    assert_eq!(
        exact_integer_remainder_obligation(target_type, cast, three, &definitions),
        Proposition::Truth,
        "the target remainder uses only its independently safe divisor",
    );
}

#[test]
fn exact_divide_remainder_cross_chain_reconstructs_carrier_total_target_prefixes() {
    let integer_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 carrier");
    let i8_count = IntegerType::new(IntegerSign::Signed, 8).expect("i8 count");
    let root_id = ValueId::new(1511).expect("divide root");
    let root = ScalarTerm::value(root_id, ScalarType::Integer(integer_type));
    let divided = ScalarTerm::value(
        ValueId::new(1512).expect("divided"),
        ScalarType::Integer(integer_type),
    );
    let remainder = ScalarTerm::value(
        ValueId::new(1513).expect("remainder"),
        ScalarType::Integer(integer_type),
    );
    let target_add = ScalarTerm::value(
        ValueId::new(1514).expect("target add"),
        ScalarType::Integer(integer_type),
    );
    let target_right = ScalarTerm::value(
        ValueId::new(1515).expect("target right"),
        ScalarType::Integer(integer_type),
    );
    let definitions = vec![
        Proposition::Equal(
            divided.clone(),
            ScalarTerm::exact_integer_divide(
                integer_type,
                root.clone(),
                ScalarTerm::integer(integer_type, IntegerValue::Unsigned(2)).expect("2u8"),
            )
            .expect("root / 2"),
        ),
        Proposition::Equal(
            remainder.clone(),
            ScalarTerm::exact_integer_remainder(
                integer_type,
                divided,
                ScalarTerm::integer(integer_type, IntegerValue::Unsigned(64)).expect("64u8"),
            )
            .expect("divided % 64"),
        ),
        Proposition::Equal(
            target_add.clone(),
            ScalarTerm::exact_integer_add(
                integer_type,
                remainder.clone(),
                ScalarTerm::integer(integer_type, IntegerValue::Unsigned(1)).expect("1u8"),
            )
            .expect("remainder + 1"),
        ),
        Proposition::Equal(
            target_right.clone(),
            ScalarTerm::exact_integer_shift_right(
                integer_type,
                i8_count,
                remainder.clone(),
                ScalarTerm::integer(i8_count, IntegerValue::Signed(1)).expect("1i8"),
            )
            .expect("remainder >> 1"),
        ),
    ];
    let parameters = BTreeSet::from([root_id]);
    assert_eq!(
        exact_integer_divide_remainder_then_affine_obligation(
            integer_type,
            target_add,
            IntegerValue::Unsigned(2),
            ExactIntegerAffineOperation::Multiply,
            &definitions,
            3,
            &parameters,
        ),
        Some(Proposition::Truth),
        "the complete [0,63] remainder hull fits the (value + 1) * 2 prefix",
    );
    assert_eq!(
        exact_integer_divide_remainder_then_affine_obligation(
            integer_type,
            remainder.clone(),
            IntegerValue::Unsigned(64),
            ExactIntegerAffineOperation::Subtract,
            &definitions,
            2,
            &parameters,
        ),
        Some(Proposition::Falsehood),
        "a hull disjoint from the current safe interval is canonical falsehood",
    );
    assert_eq!(
        exact_integer_divide_remainder_then_affine_obligation(
            integer_type,
            remainder.clone(),
            IntegerValue::Unsigned(200),
            ExactIntegerAffineOperation::Add,
            &definitions,
            2,
            &parameters,
        ),
        None,
        "partial overlap remains outside the carrier-total family",
    );
    assert_eq!(
        exact_integer_divide_remainder_then_affine_obligation(
            integer_type,
            remainder,
            IntegerValue::Unsigned(0),
            ExactIntegerAffineOperation::Multiply,
            &definitions,
            2,
            &parameters,
        ),
        Some(Proposition::Truth),
        "zero decides only the current target prefix after the full source walk",
    );
    assert_eq!(
        exact_integer_divide_remainder_then_shift_obligation(
            integer_type,
            target_right,
            2,
            &definitions,
            definitions.len(),
            &parameters,
        ),
        Some(Proposition::Truth),
        "the target-right preimage admits the whole hull before the target-left prefix",
    );
    assert_eq!(
        exact_integer_divide_remainder_then_shift_obligation(
            integer_type,
            ScalarTerm::value(
                ValueId::new(1516).expect("stale target"),
                ScalarType::Integer(integer_type),
            ),
            2,
            &definitions,
            definitions.len(),
            &parameters,
        ),
        None,
        "stale definitions cannot authorize carrier-total replay",
    );
}
