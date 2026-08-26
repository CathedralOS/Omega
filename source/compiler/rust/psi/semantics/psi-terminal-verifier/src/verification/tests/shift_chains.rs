use super::super::*;
use psi_core::{IntegerType, PropositionContext, ScalarType, ValueId};

#[test]
fn exact_shift_right_chain_counts_reconstruct_without_value_definitions() {
    let value_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 value");
    let signed_count_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8 count");
    let unsigned_count_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16 count");
    let one = ScalarTerm::integer(signed_count_type, IntegerValue::Signed(1)).expect("1i8");
    let two = ScalarTerm::integer(unsigned_count_type, IntegerValue::Unsigned(2)).expect("2u16");
    assert_eq!(
        exact_integer_shift_obligation(value_type, signed_count_type, one, &[]),
        Proposition::Truth
    );
    assert_eq!(
        exact_integer_shift_obligation(value_type, unsigned_count_type, two, &[]),
        Proposition::Truth
    );
    let negative_one =
        ScalarTerm::integer(signed_count_type, IntegerValue::Signed(-1)).expect("-1i8");
    let eight = ScalarTerm::integer(unsigned_count_type, IntegerValue::Unsigned(8)).expect("8u16");
    assert_eq!(
        exact_integer_shift_obligation(value_type, signed_count_type, negative_one, &[]),
        Proposition::Falsehood
    );
    assert_eq!(
        exact_integer_shift_obligation(value_type, unsigned_count_type, eight, &[]),
        Proposition::Falsehood
    );
}

#[test]
fn exact_mixed_shift_chain_reconstructs_alternating_prefixes_from_ordered_definitions() {
    let value_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 value");
    let signed_count_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8 count");
    let unsigned_count_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16 count");
    let root_id = ValueId::new(301).expect("root");
    let root = ScalarTerm::value(root_id, ScalarType::Integer(value_type));
    let left_one = ScalarTerm::value(
        ValueId::new(302).expect("left one"),
        ScalarType::Integer(value_type),
    );
    let right_two = ScalarTerm::value(
        ValueId::new(303).expect("right two"),
        ScalarType::Integer(value_type),
    );
    let left_three = ScalarTerm::value(
        ValueId::new(304).expect("left three"),
        ScalarType::Integer(value_type),
    );
    let one = ScalarTerm::integer(signed_count_type, IntegerValue::Signed(1)).expect("1i8");
    let two = ScalarTerm::integer(unsigned_count_type, IntegerValue::Unsigned(2)).expect("2u16");
    let three = ScalarTerm::integer(signed_count_type, IntegerValue::Signed(3)).expect("3i8");
    let definitions = vec![
        Proposition::Equal(
            left_one.clone(),
            ScalarTerm::exact_integer_shift_left(
                value_type,
                signed_count_type,
                root.clone(),
                one.clone(),
            )
            .expect("root << 1"),
        ),
        Proposition::Equal(
            right_two.clone(),
            ScalarTerm::exact_integer_shift_right(value_type, unsigned_count_type, left_one, two)
                .expect("(root << 1) >> 2"),
        ),
        Proposition::Equal(
            left_three.clone(),
            ScalarTerm::exact_integer_shift_left(value_type, signed_count_type, right_two, three)
                .expect("((root << 1) >> 2) << 3"),
        ),
    ];
    let context = PropositionContext::from_value_types((301..=305).map(|id| {
        (
            ValueId::new(id).expect("shift value"),
            ScalarType::Integer(value_type),
        )
    }))
    .expect("mixed-shift context");
    let maximum = ScalarTerm::integer(value_type, IntegerValue::Unsigned(31)).expect("31u8");
    assert_eq!(
        exact_integer_mixed_shift_chain_obligation(
            &context,
            value_type,
            left_three,
            1,
            &definitions,
            definitions.len(),
            &BTreeSet::from([root_id]),
        ),
        Some(Proposition::LessOrEqual(root.clone(), maximum)),
        "each alternating definition is replayed backward before the final left prefix",
    );

    let right = ScalarTerm::value(
        ValueId::new(305).expect("right"),
        ScalarType::Integer(value_type),
    );
    let one_right_definition = vec![Proposition::Equal(
        right.clone(),
        ScalarTerm::exact_integer_shift_right(
            value_type,
            signed_count_type,
            root.clone(),
            one.clone(),
        )
        .expect("root >> 1"),
    )];
    let maximum = ScalarTerm::integer(value_type, IntegerValue::Unsigned(31)).expect("31u8");
    assert_eq!(
        exact_integer_mixed_shift_chain_obligation(
            &context,
            value_type,
            right.clone(),
            4,
            &one_right_definition,
            one_right_definition.len(),
            &BTreeSet::from([root_id]),
        ),
        Some(Proposition::LessOrEqual(root.clone(), maximum)),
    );
    assert_eq!(
        exact_integer_mixed_shift_chain_obligation(
            &context,
            value_type,
            right,
            4,
            &one_right_definition,
            one_right_definition.len(),
            &BTreeSet::new(),
        ),
        None,
        "a local or unregistered root cannot acquire machine-parameter bounds",
    );
}

#[test]
fn exact_mixed_shift_chain_consumes_only_checked_ordered_count_landings() {
    let value_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 value");
    let u16_count_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16 count");
    let i8_count_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8 count");
    let root_id = ValueId::new(341).expect("root");
    let root = ScalarTerm::value(root_id, ScalarType::Integer(value_type));
    let first = ScalarTerm::value(
        ValueId::new(342).expect("first shift"),
        ScalarType::Integer(value_type),
    );
    let first_count = ScalarTerm::value(
        ValueId::new(343).expect("first count"),
        ScalarType::Integer(u16_count_type),
    );
    let second = ScalarTerm::value(
        ValueId::new(344).expect("second shift"),
        ScalarType::Integer(value_type),
    );
    let second_count = ScalarTerm::value(
        ValueId::new(345).expect("second count"),
        ScalarType::Integer(i8_count_type),
    );
    let target = ScalarTerm::value(
        ValueId::new(346).expect("target shift"),
        ScalarType::Integer(value_type),
    );
    let one_u16 = ScalarTerm::integer(u16_count_type, IntegerValue::Unsigned(1)).expect("1u16");
    let two_i8 = ScalarTerm::integer(i8_count_type, IntegerValue::Signed(2)).expect("2i8");
    let one_i8 = ScalarTerm::integer(i8_count_type, IntegerValue::Signed(1)).expect("1i8");
    let first_definition = Proposition::Equal(
        first.clone(),
        ScalarTerm::exact_integer_shift_left(
            value_type,
            u16_count_type,
            root.clone(),
            first_count.clone(),
        )
        .expect("root << first count"),
    );
    let second_definition = Proposition::Equal(
        second.clone(),
        ScalarTerm::exact_integer_shift_right(
            value_type,
            i8_count_type,
            first.clone(),
            second_count.clone(),
        )
        .expect("first >> second count"),
    );
    let target_definition = Proposition::Equal(
        target.clone(),
        ScalarTerm::exact_integer_shift_left(value_type, i8_count_type, second.clone(), one_i8)
            .expect("second << 1"),
    );
    let first_landing = Proposition::Equal(first_count.clone(), one_u16);
    let second_landing = Proposition::Equal(second_count.clone(), two_i8);
    let axioms = vec![
        first_landing.clone(),
        first_definition.clone(),
        second_landing.clone(),
        second_definition.clone(),
        target_definition.clone(),
    ];
    let context = PropositionContext::from_value_types([
        (root_id, ScalarType::Integer(value_type)),
        (
            ValueId::new(342).expect("first shift"),
            ScalarType::Integer(value_type),
        ),
        (
            ValueId::new(343).expect("first count"),
            ScalarType::Integer(u16_count_type),
        ),
        (
            ValueId::new(344).expect("second shift"),
            ScalarType::Integer(value_type),
        ),
        (
            ValueId::new(345).expect("second count"),
            ScalarType::Integer(i8_count_type),
        ),
        (
            ValueId::new(346).expect("target shift"),
            ScalarType::Integer(value_type),
        ),
    ])
    .expect("mixed-shift context");
    let reduce = |axioms: &[Proposition]| {
        exact_integer_mixed_shift_chain_obligation(
            &context,
            value_type,
            target.clone(),
            1,
            axioms,
            axioms.len(),
            &BTreeSet::from([root_id]),
        )
    };
    assert_eq!(
        reduce(&axioms),
        Some(Proposition::LessOrEqual(
            root.clone(),
            ScalarTerm::integer(value_type, IntegerValue::Unsigned(127)).expect("127u8"),
        )),
        "the checked left/right/left word alone drives the preimage",
    );

    assert_eq!(
        reduce(&[
            first_landing.clone(),
            first_definition.clone(),
            second_definition.clone(),
            target_definition.clone(),
        ]),
        None,
        "a missing second count landing cannot be inferred",
    );
    assert_eq!(
        reduce(&[
            first_landing.clone(),
            first_definition.clone(),
            second_definition.clone(),
            second_landing.clone(),
            target_definition.clone(),
        ]),
        None,
        "a count landing later than its definition is unavailable",
    );
    assert_eq!(
        reduce(&[
            first_landing.clone(),
            first_definition.clone(),
            Proposition::Equal(
                first_count,
                ScalarTerm::integer(u16_count_type, IntegerValue::Unsigned(2)).expect("2u16"),
            ),
            second_definition.clone(),
            target_definition.clone(),
        ]),
        None,
        "one count index cannot be reused for a distinct count identity",
    );
    assert_eq!(
        reduce(&[
            first_landing.clone(),
            second_landing.clone(),
            second_definition.clone(),
            first_definition.clone(),
            target_definition.clone(),
        ]),
        None,
        "definitions must remain ordered from root to target",
    );
    let drifted_target_definition = Proposition::Equal(
        target.clone(),
        ScalarTerm::exact_integer_shift_left(
            value_type,
            i8_count_type,
            root.clone(),
            ScalarTerm::integer(i8_count_type, IntegerValue::Signed(1)).expect("1i8"),
        )
        .expect("drifted target"),
    );
    assert_eq!(
        reduce(&[
            first_landing,
            first_definition,
            second_landing,
            second_definition,
            target_definition,
            drifted_target_definition,
        ]),
        None,
        "the latest target definition cannot drift away from the selected chain",
    );
}

#[test]
fn exact_mixed_shift_chain_handles_signed_preimages_and_stale_definitions() {
    let signed_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let count_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 count");
    let root_id = ValueId::new(311).expect("signed root");
    let signed_root = ScalarTerm::value(root_id, ScalarType::Integer(signed_type));
    let right = ScalarTerm::value(
        ValueId::new(312).expect("signed right"),
        ScalarType::Integer(signed_type),
    );
    let one = ScalarTerm::integer(count_type, IntegerValue::Unsigned(1)).expect("1u8");
    let definitions = vec![Proposition::Equal(
        right.clone(),
        ScalarTerm::exact_integer_shift_right(signed_type, count_type, signed_root.clone(), one)
            .expect("signed root >> 1"),
    )];
    let context = PropositionContext::from_value_types((311..=312).map(|id| {
        (
            ValueId::new(id).expect("shift value"),
            ScalarType::Integer(signed_type),
        )
    }))
    .expect("mixed-shift context");
    let minimum = ScalarTerm::integer(signed_type, IntegerValue::Signed(-32)).expect("-32i8");
    let maximum = ScalarTerm::integer(signed_type, IntegerValue::Signed(31)).expect("31i8");
    assert_eq!(
        exact_integer_mixed_shift_chain_obligation(
            &context,
            signed_type,
            right.clone(),
            3,
            &definitions,
            definitions.len(),
            &BTreeSet::from([root_id]),
        ),
        Some(canonical_conjunction(vec![
            Proposition::LessOrEqual(minimum, signed_root.clone()),
            Proposition::LessOrEqual(signed_root.clone(), maximum),
        ])),
    );
    assert_eq!(
        exact_integer_mixed_shift_chain_obligation(
            &context,
            signed_type,
            right,
            3,
            &[Proposition::Truth],
            1,
            &BTreeSet::from([root_id]),
        ),
        None,
        "a stale or redirected definition cannot authorize the mixed prefix",
    );

    let unsigned_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    assert_eq!(
        exact_integer_mixed_shift_preimage(
            unsigned_type,
            (0, 15),
            &ScalarTerm::exact_integer_shift_right(
                unsigned_type,
                count_type,
                ScalarTerm::integer(unsigned_type, IntegerValue::Unsigned(0)).expect("0u8"),
                ScalarTerm::integer(count_type, IntegerValue::Unsigned(4)).expect("4u8"),
            )
            .expect("unsigned right shape"),
            4,
        ),
        Ok(Some((0, 255))),
        "a right-shift preimage clips to the source carrier",
    );
}

#[test]
fn exact_mixed_shift_chain_cast_reconstructs_unsigned_and_cross_sign_preimages() {
    let source_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16 source");
    let target_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 target");
    let i8_count = IntegerType::new(IntegerSign::Signed, 8).expect("i8 count");
    let u16_count = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16 count");
    let i32_count = IntegerType::new(IntegerSign::Signed, 32).expect("i32 count");
    let root_id = ValueId::new(321).expect("root");
    let root = ScalarTerm::value(root_id, ScalarType::Integer(source_type));
    let left_one = ScalarTerm::value(
        ValueId::new(322).expect("left one"),
        ScalarType::Integer(source_type),
    );
    let right_two = ScalarTerm::value(
        ValueId::new(323).expect("right two"),
        ScalarType::Integer(source_type),
    );
    let left_three = ScalarTerm::value(
        ValueId::new(324).expect("left three"),
        ScalarType::Integer(source_type),
    );
    let one = ScalarTerm::integer(i8_count, IntegerValue::Signed(1)).expect("1i8");
    let two = ScalarTerm::integer(u16_count, IntegerValue::Unsigned(2)).expect("2u16");
    let three = ScalarTerm::integer(i32_count, IntegerValue::Signed(3)).expect("3i32");
    let definitions = vec![
        Proposition::Equal(
            left_one.clone(),
            ScalarTerm::exact_integer_shift_left(source_type, i8_count, root.clone(), one)
                .expect("root << 1"),
        ),
        Proposition::Equal(
            right_two.clone(),
            ScalarTerm::exact_integer_shift_right(source_type, u16_count, left_one, two)
                .expect("(root << 1) >> 2"),
        ),
        Proposition::Equal(
            left_three.clone(),
            ScalarTerm::exact_integer_shift_left(source_type, i32_count, right_two, three)
                .expect("((root << 1) >> 2) << 3"),
        ),
    ];
    let maximum = ScalarTerm::integer(source_type, IntegerValue::Unsigned(63)).expect("63u16");
    assert_eq!(
        exact_integer_mixed_shift_chain_cast_obligation(
            source_type,
            target_type,
            left_three,
            &definitions,
            &BTreeSet::from([root_id]),
        ),
        Some(Proposition::LessOrEqual(root.clone(), maximum)),
    );

    let signed_source = IntegerType::new(IntegerSign::Signed, 16).expect("i16 source");
    let signed_root_id = ValueId::new(325).expect("signed root");
    let signed_root = ScalarTerm::value(signed_root_id, ScalarType::Integer(signed_source));
    let signed_right = ScalarTerm::value(
        ValueId::new(326).expect("signed right"),
        ScalarType::Integer(signed_source),
    );
    let signed_left = ScalarTerm::value(
        ValueId::new(327).expect("signed left"),
        ScalarType::Integer(signed_source),
    );
    let signed_definitions = vec![
        Proposition::Equal(
            signed_right.clone(),
            ScalarTerm::exact_integer_shift_right(
                signed_source,
                i8_count,
                signed_root.clone(),
                ScalarTerm::integer(i8_count, IntegerValue::Signed(1)).expect("1i8"),
            )
            .expect("signed root >> 1"),
        ),
        Proposition::Equal(
            signed_left.clone(),
            ScalarTerm::exact_integer_shift_left(
                signed_source,
                u16_count,
                signed_right,
                ScalarTerm::integer(u16_count, IntegerValue::Unsigned(2)).expect("2u16"),
            )
            .expect("(signed root >> 1) << 2"),
        ),
    ];
    let minimum = ScalarTerm::integer(signed_source, IntegerValue::Signed(0)).expect("0i16");
    let maximum = ScalarTerm::integer(signed_source, IntegerValue::Signed(127)).expect("127i16");
    assert_eq!(
        exact_integer_mixed_shift_chain_cast_obligation(
            signed_source,
            target_type,
            signed_left,
            &signed_definitions,
            &BTreeSet::from([signed_root_id]),
        ),
        Some(canonical_conjunction(vec![
            Proposition::LessOrEqual(minimum, signed_root.clone()),
            Proposition::LessOrEqual(signed_root, maximum),
        ])),
    );
}

#[test]
fn exact_cast_then_mixed_shift_chain_reconstructs_each_left_prefix_from_source_root() {
    let source_type = IntegerType::new(IntegerSign::Signed, 16).expect("i16 source");
    let target_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 target");
    let i8_count = IntegerType::new(IntegerSign::Signed, 8).expect("i8 count");
    let u16_count = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16 count");
    let root_id = ValueId::new(331).expect("root");
    let root = ScalarTerm::value(root_id, ScalarType::Integer(source_type));
    let cast = ScalarTerm::value(
        ValueId::new(332).expect("cast"),
        ScalarType::Integer(target_type),
    );
    let left = ScalarTerm::value(
        ValueId::new(333).expect("left"),
        ScalarType::Integer(target_type),
    );
    let right = ScalarTerm::value(
        ValueId::new(334).expect("right"),
        ScalarType::Integer(target_type),
    );
    let definitions = vec![
        Proposition::Equal(
            cast.clone(),
            ScalarTerm::integer_exact_cast(source_type, target_type, root.clone())
                .expect("i16 to u8 cast"),
        ),
        Proposition::Equal(
            left.clone(),
            ScalarTerm::exact_integer_shift_left(
                target_type,
                i8_count,
                cast,
                ScalarTerm::integer(i8_count, IntegerValue::Signed(1)).expect("1i8"),
            )
            .expect("cast << 1"),
        ),
        Proposition::Equal(
            right.clone(),
            ScalarTerm::exact_integer_shift_right(
                target_type,
                u16_count,
                left,
                ScalarTerm::integer(u16_count, IntegerValue::Unsigned(2)).expect("2u16"),
            )
            .expect("(cast << 1) >> 2"),
        ),
    ];
    let minimum = ScalarTerm::integer(source_type, IntegerValue::Signed(0)).expect("0i16");
    let maximum = ScalarTerm::integer(source_type, IntegerValue::Signed(63)).expect("63i16");
    assert_eq!(
        exact_integer_cast_then_mixed_shift_chain_obligation(
            target_type,
            right.clone(),
            3,
            &definitions,
            definitions.len(),
            &BTreeSet::from([root_id]),
        ),
        Some(canonical_conjunction(vec![
            Proposition::LessOrEqual(minimum, root.clone()),
            Proposition::LessOrEqual(root.clone(), maximum),
        ])),
    );
    assert_eq!(
        exact_integer_cast_then_mixed_shift_chain_obligation(
            target_type,
            right,
            3,
            &[Proposition::Truth],
            1,
            &BTreeSet::from([root_id]),
        ),
        None,
        "a stale definition cannot authorize a post-cast mixed prefix",
    );

    let cross_source = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16 source");
    let cross_target = IntegerType::new(IntegerSign::Signed, 8).expect("i8 target");
    let cross_root_id = ValueId::new(335).expect("cross root");
    let cross_root = ScalarTerm::value(cross_root_id, ScalarType::Integer(cross_source));
    let cross_cast = ScalarTerm::value(
        ValueId::new(336).expect("cross cast"),
        ScalarType::Integer(cross_target),
    );
    let cross_right = ScalarTerm::value(
        ValueId::new(337).expect("cross right"),
        ScalarType::Integer(cross_target),
    );
    let cross_definitions = vec![
        Proposition::Equal(
            cross_cast.clone(),
            ScalarTerm::integer_exact_cast(cross_source, cross_target, cross_root.clone())
                .expect("u16 to i8 cast"),
        ),
        Proposition::Equal(
            cross_right.clone(),
            ScalarTerm::exact_integer_shift_right(
                cross_target,
                i8_count,
                cross_cast,
                ScalarTerm::integer(i8_count, IntegerValue::Signed(1)).expect("1i8"),
            )
            .expect("cast >> 1"),
        ),
    ];
    assert_eq!(
        exact_integer_cast_then_mixed_shift_chain_obligation(
            cross_target,
            cross_right,
            2,
            &cross_definitions,
            cross_definitions.len(),
            &BTreeSet::from([cross_root_id]),
        ),
        Some(Proposition::LessOrEqual(
            cross_root,
            ScalarTerm::integer(cross_source, IntegerValue::Unsigned(63)).expect("63u16"),
        )),
    );
}

#[test]
fn exact_shift_cast_shift_reconstructs_both_sides_without_importing_prefix_proofs() {
    let source_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16 source");
    let target_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 target");
    let i8_count = IntegerType::new(IntegerSign::Signed, 8).expect("i8 count");
    let u16_count = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16 count");
    let i32_count = IntegerType::new(IntegerSign::Signed, 32).expect("i32 count");
    let root_id = ValueId::new(338).expect("root");
    let root = ScalarTerm::value(root_id, ScalarType::Integer(source_type));
    let source_right = ScalarTerm::value(
        ValueId::new(339).expect("source right"),
        ScalarType::Integer(source_type),
    );
    let source_left = ScalarTerm::value(
        ValueId::new(340).expect("source left"),
        ScalarType::Integer(source_type),
    );
    let cast = ScalarTerm::value(
        ValueId::new(1341).expect("cast"),
        ScalarType::Integer(target_type),
    );
    let target_right = ScalarTerm::value(
        ValueId::new(1342).expect("target right"),
        ScalarType::Integer(target_type),
    );
    let definitions = vec![
        Proposition::Equal(
            source_right.clone(),
            ScalarTerm::exact_integer_shift_right(
                source_type,
                i8_count,
                root.clone(),
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
            cast.clone(),
            ScalarTerm::integer_exact_cast(source_type, target_type, source_left)
                .expect("u16 to u8 cast"),
        ),
        Proposition::Equal(
            target_right.clone(),
            ScalarTerm::exact_integer_shift_right(
                target_type,
                i32_count,
                cast,
                ScalarTerm::integer(i32_count, IntegerValue::Signed(1)).expect("1i32"),
            )
            .expect("cast >> 1"),
        ),
    ];
    let maximum = ScalarTerm::integer(source_type, IntegerValue::Unsigned(63)).expect("63u16");
    assert_eq!(
        exact_integer_shift_cast_shift_obligation(
            target_type,
            target_right.clone(),
            2,
            &definitions,
            definitions.len(),
            &BTreeSet::from([root_id]),
        ),
        Some(Proposition::LessOrEqual(root.clone(), maximum)),
        "the target prefix, cast, and source prefix are replayed from canonical definitions",
    );
    assert_eq!(
        exact_integer_shift_cast_shift_obligation(
            target_type,
            target_right,
            2,
            &[Proposition::Truth],
            1,
            &BTreeSet::from([root_id]),
        ),
        None,
        "stale definitions cannot authorize the sandwich",
    );

    let signed_source = IntegerType::new(IntegerSign::Signed, 16).expect("i16 source");
    let signed_root_id = ValueId::new(1343).expect("signed root");
    let signed_root = ScalarTerm::value(signed_root_id, ScalarType::Integer(signed_source));
    let signed_right = ScalarTerm::value(
        ValueId::new(1344).expect("signed right"),
        ScalarType::Integer(signed_source),
    );
    let signed_left = ScalarTerm::value(
        ValueId::new(1345).expect("signed left"),
        ScalarType::Integer(signed_source),
    );
    let signed_cast = ScalarTerm::value(
        ValueId::new(1346).expect("signed cast"),
        ScalarType::Integer(target_type),
    );
    let signed_target_right = ScalarTerm::value(
        ValueId::new(1347).expect("signed target right"),
        ScalarType::Integer(target_type),
    );
    let signed_definitions = vec![
        Proposition::Equal(
            signed_right.clone(),
            ScalarTerm::exact_integer_shift_right(
                signed_source,
                u16_count,
                signed_root.clone(),
                ScalarTerm::integer(u16_count, IntegerValue::Unsigned(1)).expect("1u16"),
            )
            .expect("signed root >> 1"),
        ),
        Proposition::Equal(
            signed_left.clone(),
            ScalarTerm::exact_integer_shift_left(
                signed_source,
                i8_count,
                signed_right,
                ScalarTerm::integer(i8_count, IntegerValue::Signed(2)).expect("2i8"),
            )
            .expect("(signed root >> 1) << 2"),
        ),
        Proposition::Equal(
            signed_cast.clone(),
            ScalarTerm::integer_exact_cast(signed_source, target_type, signed_left)
                .expect("i16 to u8 cast"),
        ),
        Proposition::Equal(
            signed_target_right.clone(),
            ScalarTerm::exact_integer_shift_right(
                target_type,
                i32_count,
                signed_cast,
                ScalarTerm::integer(i32_count, IntegerValue::Signed(1)).expect("1i32"),
            )
            .expect("signed cast >> 1"),
        ),
    ];
    let minimum = ScalarTerm::integer(signed_source, IntegerValue::Signed(0)).expect("0i16");
    let maximum = ScalarTerm::integer(signed_source, IntegerValue::Signed(63)).expect("63i16");
    assert_eq!(
        exact_integer_shift_cast_shift_obligation(
            target_type,
            signed_target_right,
            2,
            &signed_definitions,
            signed_definitions.len(),
            &BTreeSet::from([signed_root_id]),
        ),
        Some(canonical_conjunction(vec![
            Proposition::LessOrEqual(minimum, signed_root.clone()),
            Proposition::LessOrEqual(signed_root, maximum),
        ])),
    );
}
