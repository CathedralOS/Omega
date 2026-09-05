//! Focused depth-twenty-one producer, replay, atomicity, and boundary fences.

use super::*;

#[test]
fn plural_depth_twenty_one_paths_compose_depth_twenty_custody_and_retain_fences() {
    let checked = checked(
        r#"
        data Choice [copy] { case #1 Empty; case #2 Number(#1 value: u16); }
        data Leaf [copy] { #1 choice: Choice; }
        data Middle [copy] { #1 leaf: Leaf; }
        data First [copy] { #1 middle: Middle; }
        data Second [copy] { #1 first: First; }
        data Third [copy] { #1 second: Second; }
        data Fourth [copy] { #1 third: Third; }
        data Fifth [copy] { #1 fourth: Fourth; }
        data Sixth [copy] { #1 fifth: Fifth; }
        data Seventh [copy] { #1 sixth: Sixth; }
        data Eighth [copy] { #1 seventh: Seventh; }
        data Ninth [copy] { #1 eighth: Eighth; }
        data Tenth [copy] { #1 ninth: Ninth; }
        data Eleventh [copy] { #1 tenth: Tenth; }
        data Twelfth [copy] { #1 eleventh: Eleventh; }
        data Thirteenth [copy] { #1 twelfth: Twelfth; }
        data Fourteenth [copy] { #1 thirteenth: Thirteenth; }
        data Fifteenth [copy] { #1 fourteenth: Fourteenth; }
        data Sixteenth [copy] { #1 fifteenth: Fifteenth; }
        data Seventeenth [copy] { #1 sixteenth: Sixteenth; }
        data Eighteenth [copy] { #1 seventeenth: Seventeenth; }
        data Nineteenth [copy] { #1 eighteenth: Eighteenth; }
        data Outer [copy] { #1 left: Nineteenth; #2 right: Nineteenth; }

        data OuterTooDeep [copy] { #1 outer: Outer; }
        data OuterDirect [copy] { #1 nineteenth: Nineteenth; #2 direct: Choice; }
        data OuterArray [copy] { #1 nineteenth: Nineteenth; #2 choices: [Choice; 1]; }
        "#,
    );
    let plan = crate::build_layout_plan(&checked, NativeTarget::host(), &[]).unwrap();
    let definition = |name: &str| {
        checked
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == name)
            .unwrap()
    };
    let outer = definition("Outer");
    let nineteenth = definition("Nineteenth");
    let paths =
        project_conventional_record_with_depth_twenty_one_nested_sums_materialization_layout(
            &checked,
            &plan,
            outer.symbol,
        )
        .expect("the complete depth-twenty-one occurrence cohort should project");
    assert_eq!(paths.paths.len(), 2);
    assert_eq!(paths.paths[0].outer_field, "left");
    assert_eq!(paths.paths[0].outer_member_identity, Some(1));
    assert_eq!(paths.paths[1].outer_field, "right");
    assert_eq!(paths.paths[1].outer_member_identity, Some(2));
    assert_eq!(paths.paths[0].inner.paths.len(), 1);
    assert_eq!(paths.paths[1].inner.paths.len(), 1);
    assert_eq!(paths.outer_layout.offsets.as_deref(), Some(&[0, 8][..]));
    assert_eq!(paths.outer_layout.size, Some(16));

    let choice = |number: Option<u16>| match number {
        Some(value) => BuildTimeValue::Case {
            variant: "Number".into(),
            payload: vec![("value".into(), BuildTimeValue::Int(i64::from(value)))],
        },
        None => BuildTimeValue::Case {
            variant: "Empty".into(),
            payload: Vec::new(),
        },
    };
    let leaf = |choice| BuildTimeValue::Struct {
        type_name: "Leaf".into(),
        fields: vec![("choice".into(), choice)],
    };
    let middle = |choice| BuildTimeValue::Struct {
        type_name: "Middle".into(),
        fields: vec![("leaf".into(), leaf(choice))],
    };
    let first = |choice| BuildTimeValue::Struct {
        type_name: "First".into(),
        fields: vec![("middle".into(), middle(choice))],
    };
    let second = |choice| BuildTimeValue::Struct {
        type_name: "Second".into(),
        fields: vec![("first".into(), first(choice))],
    };
    let third = |choice| BuildTimeValue::Struct {
        type_name: "Third".into(),
        fields: vec![("second".into(), second(choice))],
    };
    let fourth = |choice| BuildTimeValue::Struct {
        type_name: "Fourth".into(),
        fields: vec![("third".into(), third(choice))],
    };
    let fifth = |choice| BuildTimeValue::Struct {
        type_name: "Fifth".into(),
        fields: vec![("fourth".into(), fourth(choice))],
    };
    let sixth = |choice| BuildTimeValue::Struct {
        type_name: "Sixth".into(),
        fields: vec![("fifth".into(), fifth(choice))],
    };
    let seventh = |choice| BuildTimeValue::Struct {
        type_name: "Seventh".into(),
        fields: vec![("sixth".into(), sixth(choice))],
    };
    let eighth_value = |choice| BuildTimeValue::Struct {
        type_name: "Eighth".into(),
        fields: vec![("seventh".into(), seventh(choice))],
    };
    let ninth_value = |choice| BuildTimeValue::Struct {
        type_name: "Ninth".into(),
        fields: vec![("eighth".into(), eighth_value(choice))],
    };
    let tenth_value = |choice| BuildTimeValue::Struct {
        type_name: "Tenth".into(),
        fields: vec![("ninth".into(), ninth_value(choice))],
    };
    let eleventh_value = |choice| BuildTimeValue::Struct {
        type_name: "Eleventh".into(),
        fields: vec![("tenth".into(), tenth_value(choice))],
    };
    let twelfth_value = |choice| BuildTimeValue::Struct {
        type_name: "Twelfth".into(),
        fields: vec![("eleventh".into(), eleventh_value(choice))],
    };
    let thirteenth_value = |choice| BuildTimeValue::Struct {
        type_name: "Thirteenth".into(),
        fields: vec![("twelfth".into(), twelfth_value(choice))],
    };
    let fourteenth_value = |choice| BuildTimeValue::Struct {
        type_name: "Fourteenth".into(),
        fields: vec![("thirteenth".into(), thirteenth_value(choice))],
    };
    let fifteenth_value = |choice| BuildTimeValue::Struct {
        type_name: "Fifteenth".into(),
        fields: vec![("fourteenth".into(), fourteenth_value(choice))],
    };
    let sixteenth_value = |choice| BuildTimeValue::Struct {
        type_name: "Sixteenth".into(),
        fields: vec![("fifteenth".into(), fifteenth_value(choice))],
    };
    let seventeenth_value = |choice| BuildTimeValue::Struct {
        type_name: "Seventeenth".into(),
        fields: vec![("sixteenth".into(), sixteenth_value(choice))],
    };
    let eighteenth_value = |choice| BuildTimeValue::Struct {
        type_name: "Eighteenth".into(),
        fields: vec![("seventeenth".into(), seventeenth_value(choice))],
    };
    let nineteenth_value = |choice| BuildTimeValue::Struct {
        type_name: "Nineteenth".into(),
        fields: vec![("eighteenth".into(), eighteenth_value(choice))],
    };
    let value = BuildTimeValue::Struct {
        type_name: "Outer".into(),
        fields: vec![
            ("left".into(), nineteenth_value(choice(None))),
            ("right".into(), nineteenth_value(choice(Some(0x1122)))),
        ],
    };
    let carrier = build_time_evaluation::validate_const_materializable_record_with_depth_twenty_one_nested_sums(
        &checked,
        "Outer",
        &paths,
        &value,
        ByteOrder::LittleEndian,
    )
    .expect("depth-twenty-one value custody should compose the depth-twenty carriers");
    assert_eq!(carrier.occurrences().len(), 2);
    assert_eq!(carrier.occurrences()[0].inner().occurrences().len(), 1);
    assert_eq!(carrier.occurrences()[1].inner().occurrences().len(), 1);
    let mut expected = vec![0; 16];
    expected[8..12].copy_from_slice(&1_u32.to_le_bytes());
    expected[12..14].copy_from_slice(&0x1122_u16.to_le_bytes());
    assert_eq!(carrier.bytes(), expected, "every padding byte remains zero");

    let big_endian = build_time_evaluation::validate_const_materializable_record_with_depth_twenty_one_nested_sums(
        &checked,
        "Outer",
        &paths,
        &value,
        ByteOrder::BigEndian,
    )
    .expect("the same complete cohort should stage in big-endian order");
    assert_eq!(&big_endian.bytes()[8..12], &1_u32.to_be_bytes());
    assert_eq!(&big_endian.bytes()[12..14], &0x1122_u16.to_be_bytes());
    assert_ne!(
        carrier.non_authoritative_materialization_report_fingerprint(),
        big_endian.non_authoritative_materialization_report_fingerprint()
    );

    let mut destination = [0x5a; 20];
    carrier
        .apply(&checked, &mut destination)
        .expect("complete replay should permit one atomic copy");
    assert_eq!(&destination[..16], carrier.bytes());
    assert_eq!(&destination[16..], &[0x5a; 4]);
    let mut short = [0x6b; 15];
    assert!(carrier.apply(&checked, &mut short).is_err());
    assert_eq!(short, [0x6b; 15]);

    let rejects = |mutated: &layout_plans::ConventionalDepthTwentyOneRecordSumPathsLayoutReport| {
        assert!(
            carrier
                .replay_against(&checked, "Outer", mutated, &value, ByteOrder::LittleEndian,)
                .is_err()
        );
    };
    let mut missing = paths.clone();
    missing.paths.pop();
    rejects(&missing);
    let mut extra = paths.clone();
    extra.paths.push(extra.paths[0].clone());
    rejects(&extra);
    let mut reordered = paths.clone();
    reordered.paths.swap(0, 1);
    rejects(&reordered);
    let mut wrong_outer_identity = paths.clone();
    wrong_outer_identity.paths[0].outer_member_identity = Some(2);
    rejects(&wrong_outer_identity);
    let mut wrong_inner_identity = paths.clone();
    wrong_inner_identity.paths[0].inner.paths[0].outer_member_identity = Some(2);
    rejects(&wrong_inner_identity);
    let mut wrong_leaf_geometry = paths.clone();
    wrong_leaf_geometry.paths[0].inner.paths[0].inner.paths[0]
        .inner
        .paths[0]
        .inner
        .paths[0]
        .inner
        .paths[0]
        .inner
        .paths[0]
        .inner
        .paths[0]
        .inner
        .paths[0]
        .inner
        .paths[0]
        .inner
        .paths[0]
        .inner
        .paths[0]
        .inner
        .paths[0]
        .inner
        .paths[0]
        .inner
        .paths[0]
        .inner
        .paths[0]
        .inner
        .paths[0]
        .inner
        .paths[0]
        .inner
        .paths[0]
        .inner
        .paths[0]
        .inner
        .paths[0]
        .child_sum_layouts[0]
        .layout
        .cases[1]
        .payload_fields[0]
        .offset += 1;
    rejects(&wrong_leaf_geometry);
    let mut wrong_case_ordinal = paths.clone();
    wrong_case_ordinal.paths[0].inner.paths[0].inner.paths[0]
        .inner
        .paths[0]
        .inner
        .paths[0]
        .inner
        .paths[0]
        .inner
        .paths[0]
        .inner
        .paths[0]
        .inner
        .paths[0]
        .inner
        .paths[0]
        .inner
        .paths[0]
        .inner
        .paths[0]
        .inner
        .paths[0]
        .inner
        .paths[0]
        .inner
        .paths[0]
        .inner
        .paths[0]
        .inner
        .paths[0]
        .inner
        .paths[0]
        .inner
        .paths[0]
        .inner
        .paths[0]
        .inner
        .paths[0]
        .child_sum_layouts[0]
        .layout
        .cases[1]
        .ordinal = 7;
    rejects(&wrong_case_ordinal);
    let mut wrong_child_extent = paths.clone();
    wrong_child_extent.paths[0].inner.outer_layout.size = Some(16);
    rejects(&wrong_child_extent);
    let mut wrong_child_alignment = paths.clone();
    wrong_child_alignment.paths[0].inner.outer_layout.align = 16;
    rejects(&wrong_child_alignment);
    let mut wrong_outer_geometry = paths.clone();
    wrong_outer_geometry.outer_layout.entries[1].placement =
        LayoutPlacementReport::At { offset: 4 };
    rejects(&wrong_outer_geometry);
    assert!(
        carrier
            .replay_against(&checked, "Outer", &paths, &value, ByteOrder::BigEndian)
            .is_err()
    );
    let mut changed_value = value.clone();
    let BuildTimeValue::Struct { fields, .. } = &mut changed_value else {
        unreachable!()
    };
    fields.swap(0, 1);
    assert!(
        carrier
            .replay_against(
                &checked,
                "Outer",
                &paths,
                &changed_value,
                ByteOrder::LittleEndian,
            )
            .is_err()
    );

    for name in ["Nineteenth", "OuterTooDeep", "OuterDirect", "OuterArray"] {
        assert!(
            project_conventional_record_with_depth_twenty_one_nested_sums_materialization_layout(
                &checked,
                &plan,
                definition(name).symbol,
            )
            .is_err(),
            "{name} remains outside the exact depth-twenty-one cohort"
        );
    }

    let record_fields = |name: &str| {
        let layout = unique_data_layout(&plan, definition(name).symbol, name).unwrap();
        let DataShape::Record { fields } = layout.shape else {
            unreachable!("fixture owner is a record")
        };
        fields
    };
    for owner in [
        "Outer",
        "Nineteenth",
        "Eighteenth",
        "Seventeenth",
        "Sixteenth",
        "Fifteenth",
        "Fourteenth",
        "Thirteenth",
        "Twelfth",
        "Eleventh",
        "Tenth",
        "Ninth",
        "Eighth",
        "Seventh",
        "Sixth",
        "Fifth",
        "Fourth",
        "Third",
        "Second",
        "First",
        "Middle",
        "Leaf",
    ] {
        let field = plan.fields.span_or_empty(record_fields(owner))[0].symbol;
        for placement_kind in 0..3 {
            let mut special_plan = plan.clone();
            match placement_kind {
                0 => special_plan
                    .repeated_fields
                    .push(crate::RepeatedFieldLayout {
                        field,
                        element_stride: 16,
                    }),
                1 => special_plan.bit_fields.push(crate::BitFieldLayout {
                    field,
                    fragments: Vec::new(),
                }),
                2 => special_plan
                    .stored_integers
                    .push(crate::StoredIntegerLayout {
                        field,
                        stored_width_bits: 8,
                        interpretation: layout_plans::IntegerInterpretation::Unsigned,
                        write_is_total: true,
                    }),
                _ => unreachable!(),
            }
            assert!(
                project_conventional_record_with_depth_twenty_one_nested_sums_materialization_layout(
                    &checked,
                    &special_plan,
                    outer.symbol,
                )
                .is_err(),
                "the new wrapper placement at {owner} remains fenced"
            );
        }
    }

    let recursive_type = checked
        .data_members(nineteenth)
        .iter()
        .find_map(|member| match member {
            DataMember::Field(field) if field.name.as_str() == "eighteenth" => {
                Some(field.type_reference)
            }
            DataMember::Field(_) | DataMember::Variant(_) => None,
        })
        .unwrap();
    let mut recursive_checked = checked.clone();
    recursive_checked
        .typed
        .type_reference_table
        .substitute_node(
            recursive_type,
            TypeReferenceNode::Named {
                symbol: nineteenth.symbol,
                name: nineteenth.name.clone(),
            },
        );
    assert!(
        project_conventional_record_with_depth_twenty_one_nested_sums_materialization_layout(
            &recursive_checked,
            &plan,
            outer.symbol,
        )
        .is_err(),
        "a recursive depth-twenty-one path must reject during bounded projection"
    );
    assert!(
        build_time_evaluation::validate_const_materializable_record_with_depth_twenty_one_nested_sums(
            &recursive_checked,
            "Outer",
            &paths,
            &value,
            ByteOrder::LittleEndian,
        )
        .is_err(),
        "a recursive depth-twenty-one path must reject during value replay"
    );

    assert!(
        project_conventional_record_with_depth_twenty_nested_sums_materialization_layout(
            &checked,
            &plan,
            nineteenth.symbol,
        )
        .is_ok(),
        "the unchanged plural depth-twenty API retains its prior cohort"
    );
    assert!(
        project_conventional_record_with_depth_twenty_nested_sums_materialization_layout(
            &checked,
            &plan,
            outer.symbol,
        )
        .is_err(),
        "the prior API must not widen to the new depth-twenty-one root"
    );
}
