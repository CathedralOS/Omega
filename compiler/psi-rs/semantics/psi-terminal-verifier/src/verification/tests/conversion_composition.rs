use super::super::*;
use psi_core::{IntegerType, ScalarType, ValueId};

#[test]
fn exact_cast_chain_intersects_every_carrier_without_importing_prefix_proofs() {
    let i64_type = IntegerType::new(IntegerSign::Signed, 64).expect("i64");
    let u64_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
    let i32_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let root_id = ValueId::new(1701).expect("cast-chain root");
    let root = ScalarTerm::value(root_id, ScalarType::Integer(i64_type));
    let first = ScalarTerm::value(
        ValueId::new(1702).expect("first cast"),
        ScalarType::Integer(u64_type),
    );
    let second = ScalarTerm::value(
        ValueId::new(1703).expect("second cast"),
        ScalarType::Integer(i32_type),
    );
    let first_definition = Proposition::Equal(
        first.clone(),
        ScalarTerm::integer_exact_cast(i64_type, u64_type, root.clone()).expect("i64 to u64"),
    );
    let second_definition = Proposition::Equal(
        second.clone(),
        ScalarTerm::integer_exact_cast(u64_type, i32_type, first.clone()).expect("u64 to i32"),
    );
    let definitions = vec![first_definition.clone(), second_definition.clone()];
    let parameters = BTreeSet::from([root_id]);

    assert_eq!(
        exact_integer_cast_chain_obligation(
            u64_type,
            i32_type,
            first.clone(),
            std::slice::from_ref(&first_definition),
            &parameters,
        ),
        Some(exact_integer_source_interval_obligation(
            i64_type,
            root.clone(),
            0,
            i32::MAX.into(),
        )),
        "the second cast independently reconstructs the first two carrier intersections",
    );
    assert_eq!(
        exact_integer_cast_chain_obligation(
            i32_type,
            u8_type,
            second.clone(),
            &definitions,
            &parameters,
        ),
        Some(exact_integer_source_interval_obligation(
            i64_type,
            root.clone(),
            0,
            u8::MAX.into(),
        )),
        "the third cast independently intersects every prior carrier",
    );

    let reordered = vec![second_definition, first_definition];
    assert_eq!(
        exact_integer_cast_chain_obligation(i32_type, u8_type, second, &reordered, &parameters,),
        None,
        "definition order is proof structure",
    );
    let widened = ScalarTerm::value(
        ValueId::new(1704).expect("widened cast"),
        ScalarType::Integer(i32_type),
    );
    let widening_definition = Proposition::Equal(
        widened.clone(),
        ScalarTerm::integer_exact_cast(
            u8_type,
            i32_type,
            ScalarTerm::value(root_id, ScalarType::Integer(u8_type)),
        )
        .expect("core term permits fixed exact casts"),
    );
    assert_eq!(
        exact_integer_cast_chain_obligation(
            i32_type,
            u8_type,
            widened,
            &[widening_definition],
            &BTreeSet::from([root_id]),
        ),
        None,
        "a nested widening-shaped cast is not admitted",
    );
}

#[test]
fn computed_prefix_cast_chain_replays_each_existing_source_algebra_independently() {
    let i64_type = IntegerType::new(IntegerSign::Signed, 64).expect("i64");
    let u64_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
    let i32_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
    let u32_type = IntegerType::new(IntegerSign::Unsigned, 32).expect("u32");
    let i16_type = IntegerType::new(IntegerSign::Signed, 16).expect("i16");
    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let u16_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16 count");
    let root_id = ValueId::new(1801).expect("computed cast-chain root");
    let root = ScalarTerm::value(root_id, ScalarType::Integer(i64_type));
    let product = ScalarTerm::value(
        ValueId::new(1802).expect("product"),
        ScalarType::Integer(i64_type),
    );
    let affine = ScalarTerm::value(
        ValueId::new(1803).expect("affine"),
        ScalarType::Integer(i64_type),
    );
    let first = ScalarTerm::value(
        ValueId::new(1804).expect("first cast"),
        ScalarType::Integer(u64_type),
    );
    let second = ScalarTerm::value(
        ValueId::new(1805).expect("second cast"),
        ScalarType::Integer(i32_type),
    );
    let two = ScalarTerm::integer(i64_type, IntegerValue::Signed(2)).expect("2i64");
    let one = ScalarTerm::integer(i64_type, IntegerValue::Signed(1)).expect("1i64");
    let affine_definitions = vec![
        Proposition::Equal(
            product.clone(),
            ScalarTerm::exact_integer_multiply(i64_type, root.clone(), two).expect("root * 2"),
        ),
        Proposition::Equal(
            affine.clone(),
            ScalarTerm::exact_integer_add(i64_type, product, one).expect("product + 1"),
        ),
        Proposition::Equal(
            first.clone(),
            ScalarTerm::integer_exact_cast(i64_type, u64_type, affine).expect("i64 to u64"),
        ),
        Proposition::Equal(
            second.clone(),
            ScalarTerm::integer_exact_cast(u64_type, i32_type, first).expect("u64 to i32"),
        ),
    ];
    let parameters = BTreeSet::from([root_id]);
    assert_eq!(
        exact_integer_computed_prefix_cast_chain_obligation(
            i32_type,
            u8_type,
            second,
            &affine_definitions,
            &parameters,
        ),
        Some(exact_integer_source_interval_obligation(
            i64_type,
            root.clone(),
            0,
            127,
        )),
        "the outer cast maps the intersection of every cast carrier through A*x+B",
    );

    let negative_product = ScalarTerm::value(
        ValueId::new(1811).expect("negative product"),
        ScalarType::Integer(i64_type),
    );
    let negative_first = ScalarTerm::value(
        ValueId::new(1812).expect("negative first cast"),
        ScalarType::Integer(u64_type),
    );
    let negative_second = ScalarTerm::value(
        ValueId::new(1813).expect("negative second cast"),
        ScalarType::Integer(i32_type),
    );
    let negative_definitions = vec![
        Proposition::Equal(
            negative_product.clone(),
            ScalarTerm::exact_integer_multiply(
                i64_type,
                root.clone(),
                ScalarTerm::integer(i64_type, IntegerValue::Signed(-2)).expect("-2i64"),
            )
            .expect("root * -2"),
        ),
        Proposition::Equal(
            negative_first.clone(),
            ScalarTerm::integer_exact_cast(i64_type, u64_type, negative_product)
                .expect("negative product to u64"),
        ),
        Proposition::Equal(
            negative_second.clone(),
            ScalarTerm::integer_exact_cast(u64_type, i32_type, negative_first).expect("u64 to i32"),
        ),
    ];
    assert_eq!(
        exact_integer_computed_prefix_cast_chain_obligation(
            i32_type,
            u8_type,
            negative_second,
            &negative_definitions,
            &parameters,
        ),
        Some(exact_integer_source_interval_obligation(
            i64_type,
            root.clone(),
            -127,
            0,
        )),
        "negative products reverse the complete cast-carrier intersection",
    );

    let shifted_left = ScalarTerm::value(
        ValueId::new(1821).expect("left shift"),
        ScalarType::Integer(i64_type),
    );
    let shifted_right = ScalarTerm::value(
        ValueId::new(1822).expect("right shift"),
        ScalarType::Integer(i64_type),
    );
    let shift_first = ScalarTerm::value(
        ValueId::new(1823).expect("shift first cast"),
        ScalarType::Integer(u64_type),
    );
    let shift_second = ScalarTerm::value(
        ValueId::new(1824).expect("shift second cast"),
        ScalarType::Integer(i32_type),
    );
    let shift_count = ScalarTerm::integer(u16_type, IntegerValue::Unsigned(1)).expect("1u16");
    let shift_definitions = vec![
        Proposition::Equal(
            shifted_left.clone(),
            ScalarTerm::exact_integer_shift_left(
                i64_type,
                u16_type,
                root.clone(),
                shift_count.clone(),
            )
            .expect("root << 1"),
        ),
        Proposition::Equal(
            shifted_right.clone(),
            ScalarTerm::exact_integer_shift_right(i64_type, u16_type, shifted_left, shift_count)
                .expect("shifted << 1 then >> 1"),
        ),
        Proposition::Equal(
            shift_first.clone(),
            ScalarTerm::integer_exact_cast(i64_type, u64_type, shifted_right)
                .expect("shift result to u64"),
        ),
        Proposition::Equal(
            shift_second.clone(),
            ScalarTerm::integer_exact_cast(u64_type, i32_type, shift_first).expect("u64 to i32"),
        ),
    ];
    assert_eq!(
        exact_integer_computed_prefix_cast_chain_obligation(
            i32_type,
            u8_type,
            shift_second,
            &shift_definitions,
            &parameters,
        ),
        Some(exact_integer_source_interval_obligation(
            i64_type,
            root.clone(),
            0,
            255,
        )),
        "mixed shifts replay from the complete cast-carrier intersection",
    );

    let unsigned_root_id = ValueId::new(1831).expect("unsigned DQ root");
    let unsigned_root = ScalarTerm::value(unsigned_root_id, ScalarType::Integer(u64_type));
    let divided = ScalarTerm::value(
        ValueId::new(1832).expect("divided"),
        ScalarType::Integer(u64_type),
    );
    let remainder = ScalarTerm::value(
        ValueId::new(1833).expect("remainder"),
        ScalarType::Integer(u64_type),
    );
    let dq_first = ScalarTerm::value(
        ValueId::new(1834).expect("DQ first cast"),
        ScalarType::Integer(i64_type),
    );
    let dq_second = ScalarTerm::value(
        ValueId::new(1835).expect("DQ second cast"),
        ScalarType::Integer(u32_type),
    );
    let dq_definitions = vec![
        Proposition::Equal(
            divided.clone(),
            ScalarTerm::exact_integer_divide(
                u64_type,
                unsigned_root,
                ScalarTerm::integer(u64_type, IntegerValue::Unsigned(2)).expect("2u64"),
            )
            .expect("root / 2"),
        ),
        Proposition::Equal(
            remainder.clone(),
            ScalarTerm::exact_integer_remainder(
                u64_type,
                divided,
                ScalarTerm::integer(u64_type, IntegerValue::Unsigned(3)).expect("3u64"),
            )
            .expect("divided % 3"),
        ),
        Proposition::Equal(
            dq_first.clone(),
            ScalarTerm::integer_exact_cast(u64_type, i64_type, remainder)
                .expect("DQ result to i64"),
        ),
        Proposition::Equal(
            dq_second.clone(),
            ScalarTerm::integer_exact_cast(i64_type, u32_type, dq_first).expect("i64 to u32"),
        ),
    ];
    assert_eq!(
        exact_integer_computed_prefix_cast_chain_obligation(
            u32_type,
            i16_type,
            dq_second,
            &dq_definitions,
            &BTreeSet::from([unsigned_root_id]),
        ),
        Some(Proposition::Truth),
        "the complete DQ hull fits every cast carrier",
    );

    let zero_product = ScalarTerm::value(
        ValueId::new(1841).expect("zero product"),
        ScalarType::Integer(i64_type),
    );
    let negative_constant = ScalarTerm::value(
        ValueId::new(1842).expect("negative constant"),
        ScalarType::Integer(i64_type),
    );
    let empty_first = ScalarTerm::value(
        ValueId::new(1843).expect("empty first cast"),
        ScalarType::Integer(u64_type),
    );
    let empty_second = ScalarTerm::value(
        ValueId::new(1844).expect("empty second cast"),
        ScalarType::Integer(i32_type),
    );
    let empty_definitions = vec![
        Proposition::Equal(
            zero_product.clone(),
            ScalarTerm::exact_integer_multiply(
                i64_type,
                root.clone(),
                ScalarTerm::integer(i64_type, IntegerValue::Signed(0)).expect("0i64"),
            )
            .expect("root * 0"),
        ),
        Proposition::Equal(
            negative_constant.clone(),
            ScalarTerm::exact_integer_add(
                i64_type,
                zero_product,
                ScalarTerm::integer(i64_type, IntegerValue::Signed(-1)).expect("-1i64"),
            )
            .expect("zero product - 1"),
        ),
        Proposition::Equal(
            empty_first.clone(),
            ScalarTerm::integer_exact_cast(i64_type, u64_type, negative_constant)
                .expect("negative constant to u64"),
        ),
        Proposition::Equal(
            empty_second.clone(),
            ScalarTerm::integer_exact_cast(u64_type, i32_type, empty_first).expect("u64 to i32"),
        ),
    ];
    assert_eq!(
        exact_integer_computed_prefix_cast_chain_obligation(
            i32_type,
            u8_type,
            empty_second,
            &empty_definitions,
            &parameters,
        ),
        Some(Proposition::Falsehood),
        "a mathematical empty preimage is canonical falsehood",
    );

    let mut stale_definitions = affine_definitions;
    stale_definitions.swap(0, 1);
    let stale_second = match &stale_definitions[3] {
        Proposition::Equal(left, _) => left.clone(),
        _ => unreachable!("cast definition is an equality"),
    };
    assert_eq!(
        exact_integer_computed_prefix_cast_chain_obligation(
            i32_type,
            u8_type,
            stale_second,
            &stale_definitions,
            &parameters,
        ),
        None,
        "reordered source definitions remain fail-closed",
    );
}

#[test]
fn cast_chain_then_computed_suffix_replays_each_target_algebra_independently() {
    let i64_type = IntegerType::new(IntegerSign::Signed, 64).expect("i64");
    let u64_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
    let i32_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
    let root_id = ValueId::new(1901).expect("cast-chain suffix root");
    let root = ScalarTerm::value(root_id, ScalarType::Integer(i64_type));
    let first = ScalarTerm::value(
        ValueId::new(1902).expect("first cast"),
        ScalarType::Integer(u64_type),
    );
    let second = ScalarTerm::value(
        ValueId::new(1903).expect("second cast"),
        ScalarType::Integer(i32_type),
    );
    let definitions = vec![
        Proposition::Equal(
            first.clone(),
            ScalarTerm::integer_exact_cast(i64_type, u64_type, root.clone()).expect("i64 to u64"),
        ),
        Proposition::Equal(
            second.clone(),
            ScalarTerm::integer_exact_cast(u64_type, i32_type, first).expect("u64 to i32"),
        ),
    ];
    let parameters = BTreeSet::from([root_id]);
    assert_eq!(
        exact_integer_cast_chain_then_affine_suffix_obligation(
            i32_type,
            second.clone(),
            IntegerValue::Signed(1),
            ExactIntegerAffineOperation::Add,
            &definitions,
            definitions.len(),
            &parameters,
        ),
        Some(exact_integer_source_interval_obligation(
            i64_type,
            root.clone(),
            0,
            2_147_483_646,
        )),
        "affine inversion intersects every cast carrier",
    );
    assert_eq!(
        exact_integer_cast_chain_then_signed_product_suffix_obligation(
            i32_type,
            second.clone(),
            IntegerValue::Signed(-2),
            &definitions,
            definitions.len(),
            &parameters,
        ),
        Some(exact_integer_source_interval_obligation(
            i64_type,
            root.clone(),
            0,
            1_073_741_824,
        )),
        "negative products reverse the target preimage before carrier intersection",
    );
    assert_eq!(
        exact_integer_cast_chain_then_shift_suffix_obligation(
            i32_type,
            second.clone(),
            1,
            &definitions,
            definitions.len(),
            &parameters,
        ),
        Some(exact_integer_source_interval_obligation(
            i64_type,
            root,
            0,
            1_073_741_823,
        )),
        "left-shift replay intersects the full cast-chain carrier hull",
    );
    assert_eq!(
        exact_integer_cast_chain_then_affine_suffix_obligation(
            i32_type,
            second.clone(),
            IntegerValue::Signed(0),
            ExactIntegerAffineOperation::Multiply,
            &definitions,
            definitions.len(),
            &parameters,
        ),
        Some(Proposition::Truth),
        "zero decides only its current suffix after full cast-chain validation",
    );

    let mut stale = definitions;
    stale.swap(0, 1);
    assert_eq!(
        exact_integer_cast_chain_then_affine_suffix_obligation(
            i32_type,
            second,
            IntegerValue::Signed(1),
            ExactIntegerAffineOperation::Add,
            &stale,
            stale.len(),
            &parameters,
        ),
        None,
        "reordered cast definitions remain fail-closed",
    );
}

#[test]
fn computed_prefix_cast_chain_computed_suffix_composes_existing_interval_algebras() {
    let i64_type = IntegerType::new(IntegerSign::Signed, 64).expect("i64");
    let u64_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
    let i32_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
    let root_id = ValueId::new(2001).expect("sandwich root");
    let root = ScalarTerm::value(root_id, ScalarType::Integer(i64_type));
    let source = ScalarTerm::value(
        ValueId::new(2002).expect("source affine"),
        ScalarType::Integer(i64_type),
    );
    let first = ScalarTerm::value(
        ValueId::new(2003).expect("first cast"),
        ScalarType::Integer(u64_type),
    );
    let second = ScalarTerm::value(
        ValueId::new(2004).expect("second cast"),
        ScalarType::Integer(i32_type),
    );
    let definitions = vec![
        Proposition::Equal(
            source.clone(),
            ScalarTerm::exact_integer_add(
                i64_type,
                root.clone(),
                ScalarTerm::integer(i64_type, IntegerValue::Signed(1)).expect("1i64"),
            )
            .expect("root + 1"),
        ),
        Proposition::Equal(
            first.clone(),
            ScalarTerm::integer_exact_cast(i64_type, u64_type, source).expect("i64 to u64"),
        ),
        Proposition::Equal(
            second.clone(),
            ScalarTerm::integer_exact_cast(u64_type, i32_type, first).expect("u64 to i32"),
        ),
    ];
    let parameters = BTreeSet::from([root_id]);
    assert_eq!(
        exact_integer_computed_prefix_cast_chain_interval_obligation(
            i32_type,
            second.clone(),
            (0, 100),
            &definitions,
            definitions.len(),
            &parameters,
        ),
        Some(exact_integer_source_interval_obligation(
            i64_type,
            root.clone(),
            -1,
            99,
        )),
        "target intervals cross every cast carrier before affine source inversion",
    );
    assert!(
        exact_integer_cast_chain_then_affine_suffix_obligation(
            i32_type,
            second.clone(),
            IntegerValue::Signed(1),
            ExactIntegerAffineOperation::Add,
            &definitions,
            definitions.len(),
            &parameters,
        )
        .is_some(),
        "affine target prefixes compose through the computed source",
    );
    assert!(
        exact_integer_cast_chain_then_signed_product_suffix_obligation(
            i32_type,
            second.clone(),
            IntegerValue::Signed(-2),
            &definitions,
            definitions.len(),
            &parameters,
        )
        .is_some(),
        "signed target products compose through the computed source",
    );
    assert!(
        exact_integer_cast_chain_then_shift_suffix_obligation(
            i32_type,
            second.clone(),
            1,
            &definitions,
            definitions.len(),
            &parameters,
        )
        .is_some(),
        "target left shifts compose through the computed source",
    );
    assert_eq!(
        exact_integer_cast_chain_then_affine_suffix_obligation(
            i32_type,
            second.clone(),
            IntegerValue::Signed(0),
            ExactIntegerAffineOperation::Multiply,
            &definitions,
            definitions.len(),
            &parameters,
        ),
        Some(Proposition::Truth),
        "zero decides only its target prefix after full sandwich validation",
    );

    let u32_type = IntegerType::new(IntegerSign::Unsigned, 32).expect("u32");
    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let divide_root_id = ValueId::new(2011).expect("divide root");
    let divide_root = ScalarTerm::value(divide_root_id, ScalarType::Integer(u32_type));
    let remainder = ScalarTerm::value(
        ValueId::new(2012).expect("remainder"),
        ScalarType::Integer(u32_type),
    );
    let narrow = ScalarTerm::value(
        ValueId::new(2013).expect("narrow"),
        ScalarType::Integer(u8_type),
    );
    let signed = ScalarTerm::value(
        ValueId::new(2014).expect("signed"),
        ScalarType::Integer(i8_type),
    );
    let divide_definitions = vec![
        Proposition::Equal(
            remainder.clone(),
            ScalarTerm::exact_integer_remainder(
                u32_type,
                divide_root,
                ScalarTerm::integer(u32_type, IntegerValue::Unsigned(3)).expect("3u32"),
            )
            .expect("root % 3"),
        ),
        Proposition::Equal(
            narrow.clone(),
            ScalarTerm::integer_exact_cast(u32_type, u8_type, remainder).expect("u32 to u8"),
        ),
        Proposition::Equal(
            signed.clone(),
            ScalarTerm::integer_exact_cast(u8_type, i8_type, narrow).expect("u8 to i8"),
        ),
    ];
    let divide_parameters = BTreeSet::from([divide_root_id]);
    assert_eq!(
        exact_integer_computed_prefix_cast_chain_interval_obligation(
            i8_type,
            signed.clone(),
            (0, 2),
            &divide_definitions,
            divide_definitions.len(),
            &divide_parameters,
        ),
        Some(Proposition::Truth),
        "a contained carrier-total hull is true",
    );
    assert_eq!(
        exact_integer_computed_prefix_cast_chain_interval_obligation(
            i8_type,
            signed.clone(),
            (3, 4),
            &divide_definitions,
            divide_definitions.len(),
            &divide_parameters,
        ),
        Some(Proposition::Falsehood),
        "a disjoint carrier-total hull is falsehood",
    );
    assert_eq!(
        exact_integer_computed_prefix_cast_chain_interval_obligation(
            i8_type,
            signed,
            (0, 1),
            &divide_definitions,
            divide_definitions.len(),
            &divide_parameters,
        ),
        None,
        "a partial hull overlap remains unadmitted",
    );

    let mut stale = definitions;
    stale.swap(1, 2);
    assert_eq!(
        exact_integer_cast_chain_then_affine_suffix_obligation(
            i32_type,
            second,
            IntegerValue::Signed(1),
            ExactIntegerAffineOperation::Add,
            &stale,
            stale.len(),
            &parameters,
        ),
        None,
        "reordered cast definitions remain fail-closed",
    );
}

#[test]
fn computed_prefix_widen_chain_computed_suffix_composes_existing_interval_algebras() {
    let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let i16_type = IntegerType::new(IntegerSign::Signed, 16).expect("i16");
    let i32_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
    let root_id = ValueId::new(2101).expect("widen sandwich root");
    let root = ScalarTerm::value(root_id, ScalarType::Integer(i8_type));
    let source = ScalarTerm::value(
        ValueId::new(2102).expect("source affine"),
        ScalarType::Integer(i8_type),
    );
    let first = ScalarTerm::value(
        ValueId::new(2103).expect("first widen"),
        ScalarType::Integer(i16_type),
    );
    let second = ScalarTerm::value(
        ValueId::new(2104).expect("second widen"),
        ScalarType::Integer(i32_type),
    );
    let definitions = vec![
        Proposition::Equal(
            source.clone(),
            ScalarTerm::exact_integer_add(
                i8_type,
                root.clone(),
                ScalarTerm::integer(i8_type, IntegerValue::Signed(1)).expect("1i8"),
            )
            .expect("root + 1"),
        ),
        Proposition::Equal(
            first.clone(),
            ScalarTerm::integer_widen(i8_type, i16_type, source).expect("i8 to i16"),
        ),
        Proposition::Equal(
            second.clone(),
            ScalarTerm::integer_widen(i16_type, i32_type, first).expect("i16 to i32"),
        ),
    ];
    let parameters = BTreeSet::from([root_id]);
    assert_eq!(
        exact_integer_computed_prefix_widen_chain_interval_obligation(
            i32_type,
            second.clone(),
            (0, 100),
            &definitions,
            definitions.len(),
            &parameters,
        ),
        Some(exact_integer_source_interval_obligation(
            i8_type,
            root.clone(),
            -1,
            99,
        )),
        "target intervals cross each strict widening before source inversion",
    );
    assert!(
        exact_integer_cast_chain_then_affine_suffix_obligation(
            i32_type,
            second.clone(),
            IntegerValue::Signed(1),
            ExactIntegerAffineOperation::Add,
            &definitions,
            definitions.len(),
            &parameters,
        )
        .is_some(),
        "target affine prefixes compose across the widening seam",
    );
    assert!(
        exact_integer_cast_chain_then_signed_product_suffix_obligation(
            i32_type,
            second.clone(),
            IntegerValue::Signed(-2),
            &definitions,
            definitions.len(),
            &parameters,
        )
        .is_some(),
        "target signed products compose across the widening seam",
    );
    assert!(
        exact_integer_cast_chain_then_shift_suffix_obligation(
            i32_type,
            second.clone(),
            1,
            &definitions,
            definitions.len(),
            &parameters,
        )
        .is_some(),
        "target left shifts compose across the widening seam",
    );
    assert_eq!(
        exact_integer_cast_chain_then_affine_suffix_obligation(
            i32_type,
            second.clone(),
            IntegerValue::Signed(0),
            ExactIntegerAffineOperation::Multiply,
            &definitions,
            definitions.len(),
            &parameters,
        ),
        Some(Proposition::Truth),
        "zero decides only its target prefix after full widening-shape validation",
    );

    let product_root_id = ValueId::new(2111).expect("product root");
    let product_root = ScalarTerm::value(product_root_id, ScalarType::Integer(i8_type));
    let product = ScalarTerm::value(
        ValueId::new(2112).expect("signed product"),
        ScalarType::Integer(i8_type),
    );
    let product_widened = ScalarTerm::value(
        ValueId::new(2113).expect("product widened"),
        ScalarType::Integer(i16_type),
    );
    let product_definitions = vec![
        Proposition::Equal(
            product.clone(),
            ScalarTerm::exact_integer_multiply(
                i8_type,
                product_root,
                ScalarTerm::integer(i8_type, IntegerValue::Signed(-2)).expect("-2i8"),
            )
            .expect("root * -2"),
        ),
        Proposition::Equal(
            product_widened.clone(),
            ScalarTerm::integer_widen(i8_type, i16_type, product).expect("i8 to i16"),
        ),
    ];
    assert!(
        exact_integer_computed_prefix_widen_chain_interval_obligation(
            i16_type,
            product_widened,
            (-100, 100),
            &product_definitions,
            product_definitions.len(),
            &BTreeSet::from([product_root_id]),
        )
        .is_some(),
        "negative source products retain reversed inverse replay",
    );

    let shift_root_id = ValueId::new(2121).expect("shift root");
    let shift_root = ScalarTerm::value(shift_root_id, ScalarType::Integer(u8_type));
    let shifted = ScalarTerm::value(
        ValueId::new(2122).expect("shifted"),
        ScalarType::Integer(u8_type),
    );
    let shift_widened = ScalarTerm::value(
        ValueId::new(2123).expect("shift widened"),
        ScalarType::Integer(i16_type),
    );
    let shift_definitions = vec![
        Proposition::Equal(
            shifted.clone(),
            ScalarTerm::exact_integer_shift_right(
                u8_type,
                u8_type,
                shift_root,
                ScalarTerm::integer(u8_type, IntegerValue::Unsigned(1)).expect("1u8"),
            )
            .expect("root >> 1"),
        ),
        Proposition::Equal(
            shift_widened.clone(),
            ScalarTerm::integer_widen(u8_type, i16_type, shifted).expect("u8 to i16"),
        ),
    ];
    assert!(
        exact_integer_computed_prefix_widen_chain_interval_obligation(
            i16_type,
            shift_widened,
            (0, 100),
            &shift_definitions,
            shift_definitions.len(),
            &BTreeSet::from([shift_root_id]),
        )
        .is_some(),
        "source shifts retain their ordered inverse replay",
    );

    let divide_root_id = ValueId::new(2131).expect("divide root");
    let divide_root = ScalarTerm::value(divide_root_id, ScalarType::Integer(u8_type));
    let remainder = ScalarTerm::value(
        ValueId::new(2132).expect("remainder"),
        ScalarType::Integer(u8_type),
    );
    let remainder_widened = ScalarTerm::value(
        ValueId::new(2133).expect("remainder widened"),
        ScalarType::Integer(i16_type),
    );
    let divide_definitions = vec![
        Proposition::Equal(
            remainder.clone(),
            ScalarTerm::exact_integer_remainder(
                u8_type,
                divide_root,
                ScalarTerm::integer(u8_type, IntegerValue::Unsigned(3)).expect("3u8"),
            )
            .expect("root % 3"),
        ),
        Proposition::Equal(
            remainder_widened.clone(),
            ScalarTerm::integer_widen(u8_type, i16_type, remainder).expect("u8 to i16"),
        ),
    ];
    let divide_parameters = BTreeSet::from([divide_root_id]);
    for (interval, expected) in [
        ((0, 2), Some(Proposition::Truth)),
        ((3, 4), Some(Proposition::Falsehood)),
        ((0, 1), None),
    ] {
        assert_eq!(
            exact_integer_computed_prefix_widen_chain_interval_obligation(
                i16_type,
                remainder_widened.clone(),
                interval,
                &divide_definitions,
                divide_definitions.len(),
                &divide_parameters,
            ),
            expected,
        );
    }

    let mut stale = definitions;
    stale.swap(1, 2);
    assert_eq!(
        exact_integer_cast_chain_then_affine_suffix_obligation(
            i32_type,
            second,
            IntegerValue::Signed(1),
            ExactIntegerAffineOperation::Add,
            &stale,
            stale.len(),
            &parameters,
        ),
        None,
        "reordered widening definitions remain fail-closed",
    );
}

#[test]
fn computed_prefix_mixed_conversion_chain_computed_suffix_replays_every_edge_independently() {
    let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let i16_type = IntegerType::new(IntegerSign::Signed, 16).expect("i16");
    let u16_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16");
    let i32_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
    let root_id = ValueId::new(2141).expect("mixed conversion root");
    let root = ScalarTerm::value(root_id, ScalarType::Integer(i16_type));
    let source = ScalarTerm::value(
        ValueId::new(2142).expect("source affine"),
        ScalarType::Integer(i16_type),
    );
    let widened = ScalarTerm::value(
        ValueId::new(2143).expect("widened"),
        ScalarType::Integer(i32_type),
    );
    let narrowed = ScalarTerm::value(
        ValueId::new(2144).expect("narrowed"),
        ScalarType::Integer(i16_type),
    );
    let definitions = vec![
        Proposition::Equal(
            source.clone(),
            ScalarTerm::exact_integer_add(
                i16_type,
                root.clone(),
                ScalarTerm::integer(i16_type, IntegerValue::Signed(1)).expect("1i16"),
            )
            .expect("root + 1"),
        ),
        Proposition::Equal(
            widened.clone(),
            ScalarTerm::integer_widen(i16_type, i32_type, source).expect("i16 to i32"),
        ),
        Proposition::Equal(
            narrowed.clone(),
            ScalarTerm::integer_exact_cast(i32_type, i16_type, widened.clone())
                .expect("i32 to i16"),
        ),
    ];
    let parameters = BTreeSet::from([root_id]);
    assert_eq!(
        exact_integer_computed_prefix_mixed_conversion_chain_cast_obligation(
            i32_type,
            i16_type,
            widened,
            &definitions[..2],
            &parameters,
        ),
        Some(exact_integer_source_interval_obligation(
            i16_type,
            root.clone(),
            i16::MIN.into(),
            (i16::MAX - 1).into(),
        )),
        "the partial cast replays the prior widen and source affine independently",
    );
    assert_eq!(
        exact_integer_computed_prefix_mixed_conversion_chain_interval_obligation(
            i16_type,
            narrowed.clone(),
            (0, 100),
            &definitions,
            definitions.len(),
            &parameters,
        ),
        Some(exact_integer_source_interval_obligation(
            i16_type,
            root.clone(),
            -1,
            99,
        )),
    );
    assert!(
        exact_integer_cast_chain_then_affine_suffix_obligation(
            i16_type,
            narrowed.clone(),
            IntegerValue::Signed(1),
            ExactIntegerAffineOperation::Add,
            &definitions,
            definitions.len(),
            &parameters,
        )
        .is_some(),
        "target affine algebra composes with the heterogeneous conversion spine",
    );
    assert!(
        exact_integer_cast_chain_then_signed_product_suffix_obligation(
            i16_type,
            narrowed.clone(),
            IntegerValue::Signed(-2),
            &definitions,
            definitions.len(),
            &parameters,
        )
        .is_some(),
        "target signed-product algebra composes with the heterogeneous conversion spine",
    );
    assert!(
        exact_integer_cast_chain_then_shift_suffix_obligation(
            i16_type,
            narrowed,
            1,
            &definitions,
            definitions.len(),
            &parameters,
        )
        .is_some(),
        "target shift algebra composes with the heterogeneous conversion spine",
    );

    let alternating_root_id = ValueId::new(2151).expect("alternating root");
    let alternating_root = ScalarTerm::value(alternating_root_id, ScalarType::Integer(i8_type));
    let alternating_source = ScalarTerm::value(
        ValueId::new(2152).expect("alternating source"),
        ScalarType::Integer(i8_type),
    );
    let alternating_first = ScalarTerm::value(
        ValueId::new(2153).expect("alternating first widen"),
        ScalarType::Integer(i16_type),
    );
    let alternating_second = ScalarTerm::value(
        ValueId::new(2154).expect("alternating cast"),
        ScalarType::Integer(u8_type),
    );
    let alternating_third = ScalarTerm::value(
        ValueId::new(2155).expect("alternating second widen"),
        ScalarType::Integer(i16_type),
    );
    let alternating_fourth = ScalarTerm::value(
        ValueId::new(2156).expect("alternating second cast"),
        ScalarType::Integer(u8_type),
    );
    let alternating_definitions = vec![
        Proposition::Equal(
            alternating_source.clone(),
            ScalarTerm::exact_integer_add(
                i8_type,
                alternating_root,
                ScalarTerm::integer(i8_type, IntegerValue::Signed(1)).expect("1i8"),
            )
            .expect("root + 1"),
        ),
        Proposition::Equal(
            alternating_first.clone(),
            ScalarTerm::integer_widen(i8_type, i16_type, alternating_source).expect("i8 to i16"),
        ),
        Proposition::Equal(
            alternating_second.clone(),
            ScalarTerm::integer_exact_cast(i16_type, u8_type, alternating_first)
                .expect("i16 to u8"),
        ),
        Proposition::Equal(
            alternating_third.clone(),
            ScalarTerm::integer_widen(u8_type, i16_type, alternating_second).expect("u8 to i16"),
        ),
        Proposition::Equal(
            alternating_fourth.clone(),
            ScalarTerm::integer_exact_cast(i16_type, u8_type, alternating_third.clone())
                .expect("i16 to u8"),
        ),
    ];
    assert!(
        exact_integer_computed_prefix_mixed_conversion_chain_cast_obligation(
            i16_type,
            u8_type,
            alternating_third,
            &alternating_definitions[..4],
            &BTreeSet::from([alternating_root_id]),
        )
        .is_some(),
        "each later cast replays every preceding alternating edge independently",
    );
    assert!(
        exact_integer_computed_prefix_mixed_conversion_chain_interval_obligation(
            u8_type,
            alternating_fourth.clone(),
            (0, 100),
            &alternating_definitions,
            alternating_definitions.len(),
            &BTreeSet::from([alternating_root_id]),
        )
        .is_some(),
        "an alternating widen-cast-widen word replays as one ordered spine",
    );

    let divide_root_id = ValueId::new(2161).expect("divide root");
    let divide_root = ScalarTerm::value(divide_root_id, ScalarType::Integer(u16_type));
    let remainder = ScalarTerm::value(
        ValueId::new(2162).expect("remainder"),
        ScalarType::Integer(u16_type),
    );
    let remainder_cast = ScalarTerm::value(
        ValueId::new(2163).expect("remainder cast"),
        ScalarType::Integer(i16_type),
    );
    let remainder_widened = ScalarTerm::value(
        ValueId::new(2164).expect("remainder widened"),
        ScalarType::Integer(i32_type),
    );
    let divide_definitions = vec![
        Proposition::Equal(
            remainder.clone(),
            ScalarTerm::exact_integer_remainder(
                u16_type,
                divide_root,
                ScalarTerm::integer(u16_type, IntegerValue::Unsigned(3)).expect("3u16"),
            )
            .expect("root % 3"),
        ),
        Proposition::Equal(
            remainder_cast.clone(),
            ScalarTerm::integer_exact_cast(u16_type, i16_type, remainder).expect("u16 to i16"),
        ),
        Proposition::Equal(
            remainder_widened.clone(),
            ScalarTerm::integer_widen(i16_type, i32_type, remainder_cast).expect("i16 to i32"),
        ),
    ];
    let divide_parameters = BTreeSet::from([divide_root_id]);
    for (requested, expected) in [
        ((0, 2), Some(Proposition::Truth)),
        ((3, 4), Some(Proposition::Falsehood)),
        ((0, 1), None),
    ] {
        assert_eq!(
            exact_integer_computed_prefix_mixed_conversion_chain_interval_obligation(
                i32_type,
                remainder_widened.clone(),
                requested,
                &divide_definitions,
                divide_definitions.len(),
                &divide_parameters,
            ),
            expected,
        );
    }

    let wide_divide_root_id = ValueId::new(2171).expect("wide divide root");
    let wide_divide_root = ScalarTerm::value(wide_divide_root_id, ScalarType::Integer(i16_type));
    let wide_divide = ScalarTerm::value(
        ValueId::new(2172).expect("wide divide"),
        ScalarType::Integer(i16_type),
    );
    let wide_divide_widened = ScalarTerm::value(
        ValueId::new(2173).expect("wide divide widened"),
        ScalarType::Integer(i32_type),
    );
    let wide_divide_definitions = vec![
        Proposition::Equal(
            wide_divide.clone(),
            ScalarTerm::exact_integer_divide(
                i16_type,
                wide_divide_root,
                ScalarTerm::integer(i16_type, IntegerValue::Signed(2)).expect("2i16"),
            )
            .expect("root / 2"),
        ),
        Proposition::Equal(
            wide_divide_widened.clone(),
            ScalarTerm::integer_widen(i16_type, i32_type, wide_divide).expect("i16 to i32"),
        ),
    ];
    assert_eq!(
        exact_integer_computed_prefix_mixed_conversion_chain_cast_obligation(
            i32_type,
            i8_type,
            wide_divide_widened,
            &wide_divide_definitions,
            &BTreeSet::from([wide_divide_root_id]),
        ),
        None,
        "a partial D hull never becomes cast authority or falsehood",
    );

    let mut stale = alternating_definitions;
    stale.swap(1, 2);
    assert_eq!(
        exact_integer_computed_prefix_mixed_conversion_chain_interval_obligation(
            u8_type,
            alternating_fourth,
            (0, 100),
            &stale,
            stale.len(),
            &BTreeSet::from([alternating_root_id]),
        ),
        None,
        "reordered conversion definitions remain fail-closed",
    );
}
