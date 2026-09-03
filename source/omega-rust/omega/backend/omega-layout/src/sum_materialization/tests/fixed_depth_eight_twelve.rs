use super::*;

#[test]
fn plural_depth_eight_paths_compose_depth_seven_custody_and_retain_fences() {
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
        data Outer [copy] { #1 left: Sixth; #2 right: Sixth; }

        data OuterTooDeep [copy] { #1 outer: Outer; }
        data OuterDirect [copy] { #1 sixth: Sixth; #2 direct: Choice; }
        data OuterArray [copy] { #1 sixth: Sixth; #2 choices: [Choice; 1]; }
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
    let sixth = definition("Sixth");
    let paths = project_conventional_record_with_depth_eight_nested_sums_materialization_layout(
        &checked,
        &plan,
        outer.symbol,
    )
    .expect("the complete depth-eight occurrence cohort should project");
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
    let fourth_value = |choice| BuildTimeValue::Struct {
        type_name: "Fourth".into(),
        fields: vec![("third".into(), third(choice))],
    };
    let fifth_value = |choice| BuildTimeValue::Struct {
        type_name: "Fifth".into(),
        fields: vec![("fourth".into(), fourth_value(choice))],
    };
    let sixth_value = |choice| BuildTimeValue::Struct {
        type_name: "Sixth".into(),
        fields: vec![("fifth".into(), fifth_value(choice))],
    };
    let value = BuildTimeValue::Struct {
        type_name: "Outer".into(),
        fields: vec![
            ("left".into(), sixth_value(choice(None))),
            ("right".into(), sixth_value(choice(Some(0x1122)))),
        ],
    };
    let carrier = validate_const_materializable_record_with_depth_eight_nested_sums(
        &checked,
        "Outer",
        &paths,
        &value,
        ByteOrder::LittleEndian,
    )
    .expect("depth-eight value custody should compose the depth-seven carriers");
    assert_eq!(carrier.occurrences().len(), 2);
    assert_eq!(carrier.occurrences()[0].inner().occurrences().len(), 1);
    assert_eq!(carrier.occurrences()[1].inner().occurrences().len(), 1);
    let mut expected = vec![0; 16];
    expected[8..12].copy_from_slice(&1_u32.to_le_bytes());
    expected[12..14].copy_from_slice(&0x1122_u16.to_le_bytes());
    assert_eq!(carrier.bytes(), expected, "every padding byte remains zero");

    let big_endian = validate_const_materializable_record_with_depth_eight_nested_sums(
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

    let rejects = |mutated: &psi_layout_plans::ConventionalDepthEightRecordSumPathsLayoutReport| {
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

    for name in ["Sixth", "OuterTooDeep", "OuterDirect", "OuterArray"] {
        assert!(
            project_conventional_record_with_depth_eight_nested_sums_materialization_layout(
                &checked,
                &plan,
                definition(name).symbol,
            )
            .is_err(),
            "{name} remains outside the exact depth-eight cohort"
        );
    }

    let record_fields = |name: &str| {
        let layout = unique_data_layout(&plan, definition(name).symbol, name).unwrap();
        let DataShape::Record { fields } = layout.shape else {
            unreachable!("fixture owner is a record")
        };
        fields
    };
    for (owner, index) in [
        ("Outer", 0),
        ("Sixth", 0),
        ("Fifth", 0),
        ("Fourth", 0),
        ("Third", 0),
        ("Second", 0),
        ("First", 0),
        ("Middle", 0),
        ("Leaf", 0),
    ] {
        let field = plan.fields.span_or_empty(record_fields(owner))[index].symbol;
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
                        interpretation: psi_layout_plans::IntegerInterpretation::Unsigned,
                        write_is_total: true,
                    }),
                _ => unreachable!(),
            }
            assert!(
                project_conventional_record_with_depth_eight_nested_sums_materialization_layout(
                    &checked,
                    &special_plan,
                    outer.symbol,
                )
                .is_err(),
                "repeated, fragment, and stored placement at {owner} remains fenced"
            );
        }
    }

    let recursive_type = checked
        .data_members(sixth)
        .iter()
        .find_map(|member| match member {
            DataMember::Field(field) if field.name.as_str() == "fifth" => {
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
                symbol: sixth.symbol,
                name: sixth.name.clone(),
            },
        );
    assert!(
        project_conventional_record_with_depth_eight_nested_sums_materialization_layout(
            &recursive_checked,
            &plan,
            outer.symbol,
        )
        .is_err(),
        "a recursive depth-eight path must reject during bounded projection"
    );
    assert!(
        validate_const_materializable_record_with_depth_eight_nested_sums(
            &recursive_checked,
            "Outer",
            &paths,
            &value,
            ByteOrder::LittleEndian,
        )
        .is_err(),
        "a recursive depth-eight path must reject during value replay"
    );

    assert!(
        project_conventional_record_with_depth_seven_nested_sums_materialization_layout(
            &checked,
            &plan,
            sixth.symbol,
        )
        .is_ok(),
        "the unchanged plural depth-seven API retains its prior cohort"
    );
    assert!(
        project_conventional_record_with_depth_seven_nested_sums_materialization_layout(
            &checked,
            &plan,
            outer.symbol,
        )
        .is_err(),
        "the prior API must not widen to the new depth-eight root"
    );
}

#[test]
fn plural_depth_nine_paths_compose_depth_eight_custody_and_retain_fences() {
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
        data Outer [copy] { #1 left: Seventh; #2 right: Seventh; }

        data OuterTooDeep [copy] { #1 outer: Outer; }
        data OuterDirect [copy] { #1 seventh: Seventh; #2 direct: Choice; }
        data OuterArray [copy] { #1 seventh: Seventh; #2 choices: [Choice; 1]; }
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
    let seventh = definition("Seventh");
    let paths = project_conventional_record_with_depth_nine_nested_sums_materialization_layout(
        &checked,
        &plan,
        outer.symbol,
    )
    .expect("the complete depth-nine occurrence cohort should project");
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
    let fourth_value = |choice| BuildTimeValue::Struct {
        type_name: "Fourth".into(),
        fields: vec![("third".into(), third(choice))],
    };
    let fifth_value = |choice| BuildTimeValue::Struct {
        type_name: "Fifth".into(),
        fields: vec![("fourth".into(), fourth_value(choice))],
    };
    let sixth_value = |choice| BuildTimeValue::Struct {
        type_name: "Sixth".into(),
        fields: vec![("fifth".into(), fifth_value(choice))],
    };
    let seventh_value = |choice| BuildTimeValue::Struct {
        type_name: "Seventh".into(),
        fields: vec![("sixth".into(), sixth_value(choice))],
    };
    let value = BuildTimeValue::Struct {
        type_name: "Outer".into(),
        fields: vec![
            ("left".into(), seventh_value(choice(None))),
            ("right".into(), seventh_value(choice(Some(0x1122)))),
        ],
    };
    let carrier = validate_const_materializable_record_with_depth_nine_nested_sums(
        &checked,
        "Outer",
        &paths,
        &value,
        ByteOrder::LittleEndian,
    )
    .expect("depth-nine value custody should compose the depth-eight carriers");
    assert_eq!(carrier.occurrences().len(), 2);
    assert_eq!(carrier.occurrences()[0].inner().occurrences().len(), 1);
    assert_eq!(carrier.occurrences()[1].inner().occurrences().len(), 1);
    let mut expected = vec![0; 16];
    expected[8..12].copy_from_slice(&1_u32.to_le_bytes());
    expected[12..14].copy_from_slice(&0x1122_u16.to_le_bytes());
    assert_eq!(carrier.bytes(), expected, "every padding byte remains zero");

    let big_endian = validate_const_materializable_record_with_depth_nine_nested_sums(
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

    let rejects = |mutated: &psi_layout_plans::ConventionalDepthNineRecordSumPathsLayoutReport| {
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

    for name in ["Seventh", "OuterTooDeep", "OuterDirect", "OuterArray"] {
        assert!(
            project_conventional_record_with_depth_nine_nested_sums_materialization_layout(
                &checked,
                &plan,
                definition(name).symbol,
            )
            .is_err(),
            "{name} remains outside the exact depth-nine cohort"
        );
    }

    let record_fields = |name: &str| {
        let layout = unique_data_layout(&plan, definition(name).symbol, name).unwrap();
        let DataShape::Record { fields } = layout.shape else {
            unreachable!("fixture owner is a record")
        };
        fields
    };
    for (owner, index) in [
        ("Outer", 0),
        ("Seventh", 0),
        ("Sixth", 0),
        ("Fifth", 0),
        ("Fourth", 0),
        ("Third", 0),
        ("Second", 0),
        ("First", 0),
        ("Middle", 0),
        ("Leaf", 0),
    ] {
        let field = plan.fields.span_or_empty(record_fields(owner))[index].symbol;
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
                        interpretation: psi_layout_plans::IntegerInterpretation::Unsigned,
                        write_is_total: true,
                    }),
                _ => unreachable!(),
            }
            assert!(
                project_conventional_record_with_depth_nine_nested_sums_materialization_layout(
                    &checked,
                    &special_plan,
                    outer.symbol,
                )
                .is_err(),
                "repeated, fragment, and stored placement at {owner} remains fenced"
            );
        }
    }

    let recursive_type = checked
        .data_members(seventh)
        .iter()
        .find_map(|member| match member {
            DataMember::Field(field) if field.name.as_str() == "sixth" => {
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
                symbol: seventh.symbol,
                name: seventh.name.clone(),
            },
        );
    assert!(
        project_conventional_record_with_depth_nine_nested_sums_materialization_layout(
            &recursive_checked,
            &plan,
            outer.symbol,
        )
        .is_err(),
        "a recursive depth-nine path must reject during bounded projection"
    );
    assert!(
        validate_const_materializable_record_with_depth_nine_nested_sums(
            &recursive_checked,
            "Outer",
            &paths,
            &value,
            ByteOrder::LittleEndian,
        )
        .is_err(),
        "a recursive depth-nine path must reject during value replay"
    );

    assert!(
        project_conventional_record_with_depth_eight_nested_sums_materialization_layout(
            &checked,
            &plan,
            seventh.symbol,
        )
        .is_ok(),
        "the unchanged plural depth-eight API retains its prior cohort"
    );
    assert!(
        project_conventional_record_with_depth_eight_nested_sums_materialization_layout(
            &checked,
            &plan,
            outer.symbol,
        )
        .is_err(),
        "the prior API must not widen to the new depth-nine root"
    );
}

#[test]
fn plural_depth_ten_paths_compose_depth_nine_custody_and_retain_fences() {
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
        data Outer [copy] { #1 left: Eighth; #2 right: Eighth; }

        data OuterTooDeep [copy] { #1 outer: Outer; }
        data OuterDirect [copy] { #1 eighth: Eighth; #2 direct: Choice; }
        data OuterArray [copy] { #1 eighth: Eighth; #2 choices: [Choice; 1]; }
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
    let eighth = definition("Eighth");
    let paths = project_conventional_record_with_depth_ten_nested_sums_materialization_layout(
        &checked,
        &plan,
        outer.symbol,
    )
    .expect("the complete depth-ten occurrence cohort should project");
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
    let value = BuildTimeValue::Struct {
        type_name: "Outer".into(),
        fields: vec![
            ("left".into(), eighth_value(choice(None))),
            ("right".into(), eighth_value(choice(Some(0x1122)))),
        ],
    };
    let carrier = validate_const_materializable_record_with_depth_ten_nested_sums(
        &checked,
        "Outer",
        &paths,
        &value,
        ByteOrder::LittleEndian,
    )
    .expect("depth-ten value custody should compose the depth-nine carriers");
    assert_eq!(carrier.occurrences().len(), 2);
    assert_eq!(carrier.occurrences()[0].inner().occurrences().len(), 1);
    assert_eq!(carrier.occurrences()[1].inner().occurrences().len(), 1);
    let mut expected = vec![0; 16];
    expected[8..12].copy_from_slice(&1_u32.to_le_bytes());
    expected[12..14].copy_from_slice(&0x1122_u16.to_le_bytes());
    assert_eq!(carrier.bytes(), expected, "every padding byte remains zero");

    let big_endian = validate_const_materializable_record_with_depth_ten_nested_sums(
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

    let rejects = |mutated: &psi_layout_plans::ConventionalDepthTenRecordSumPathsLayoutReport| {
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

    for name in ["Eighth", "OuterTooDeep", "OuterDirect", "OuterArray"] {
        assert!(
            project_conventional_record_with_depth_ten_nested_sums_materialization_layout(
                &checked,
                &plan,
                definition(name).symbol,
            )
            .is_err(),
            "{name} remains outside the exact depth-ten cohort"
        );
    }

    let record_fields = |name: &str| {
        let layout = unique_data_layout(&plan, definition(name).symbol, name).unwrap();
        let DataShape::Record { fields } = layout.shape else {
            unreachable!("fixture owner is a record")
        };
        fields
    };
    for owner in ["Outer", "Eighth"] {
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
                        interpretation: psi_layout_plans::IntegerInterpretation::Unsigned,
                        write_is_total: true,
                    }),
                _ => unreachable!(),
            }
            assert!(
                project_conventional_record_with_depth_ten_nested_sums_materialization_layout(
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
        .data_members(eighth)
        .iter()
        .find_map(|member| match member {
            DataMember::Field(field) if field.name.as_str() == "seventh" => {
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
                symbol: eighth.symbol,
                name: eighth.name.clone(),
            },
        );
    assert!(
        project_conventional_record_with_depth_ten_nested_sums_materialization_layout(
            &recursive_checked,
            &plan,
            outer.symbol,
        )
        .is_err(),
        "a recursive depth-ten path must reject during bounded projection"
    );
    assert!(
        validate_const_materializable_record_with_depth_ten_nested_sums(
            &recursive_checked,
            "Outer",
            &paths,
            &value,
            ByteOrder::LittleEndian,
        )
        .is_err(),
        "a recursive depth-ten path must reject during value replay"
    );

    assert!(
        project_conventional_record_with_depth_nine_nested_sums_materialization_layout(
            &checked,
            &plan,
            eighth.symbol,
        )
        .is_ok(),
        "the unchanged plural depth-nine API retains its prior cohort"
    );
    assert!(
        project_conventional_record_with_depth_nine_nested_sums_materialization_layout(
            &checked,
            &plan,
            outer.symbol,
        )
        .is_err(),
        "the prior API must not widen to the new depth-ten root"
    );
}

#[test]
fn plural_depth_eleven_paths_compose_depth_ten_custody_and_retain_fences() {
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
        data Outer [copy] { #1 left: Ninth; #2 right: Ninth; }

        data OuterTooDeep [copy] { #1 outer: Outer; }
        data OuterDirect [copy] { #1 ninth: Ninth; #2 direct: Choice; }
        data OuterArray [copy] { #1 ninth: Ninth; #2 choices: [Choice; 1]; }
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
    let ninth = definition("Ninth");
    let paths = project_conventional_record_with_depth_eleven_nested_sums_materialization_layout(
        &checked,
        &plan,
        outer.symbol,
    )
    .expect("the complete depth-eleven occurrence cohort should project");
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
    let value = BuildTimeValue::Struct {
        type_name: "Outer".into(),
        fields: vec![
            ("left".into(), ninth_value(choice(None))),
            ("right".into(), ninth_value(choice(Some(0x1122)))),
        ],
    };
    let carrier = validate_const_materializable_record_with_depth_eleven_nested_sums(
        &checked,
        "Outer",
        &paths,
        &value,
        ByteOrder::LittleEndian,
    )
    .expect("depth-eleven value custody should compose the depth-ten carriers");
    assert_eq!(carrier.occurrences().len(), 2);
    assert_eq!(carrier.occurrences()[0].inner().occurrences().len(), 1);
    assert_eq!(carrier.occurrences()[1].inner().occurrences().len(), 1);
    let mut expected = vec![0; 16];
    expected[8..12].copy_from_slice(&1_u32.to_le_bytes());
    expected[12..14].copy_from_slice(&0x1122_u16.to_le_bytes());
    assert_eq!(carrier.bytes(), expected, "every padding byte remains zero");

    let big_endian = validate_const_materializable_record_with_depth_eleven_nested_sums(
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

    let rejects =
        |mutated: &psi_layout_plans::ConventionalDepthElevenRecordSumPathsLayoutReport| {
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

    for name in ["Ninth", "OuterTooDeep", "OuterDirect", "OuterArray"] {
        assert!(
            project_conventional_record_with_depth_eleven_nested_sums_materialization_layout(
                &checked,
                &plan,
                definition(name).symbol,
            )
            .is_err(),
            "{name} remains outside the exact depth-eleven cohort"
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
        "Outer", "Ninth", "Eighth", "Seventh", "Sixth", "Fifth", "Fourth", "Third", "Second",
        "First", "Middle", "Leaf",
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
                        interpretation: psi_layout_plans::IntegerInterpretation::Unsigned,
                        write_is_total: true,
                    }),
                _ => unreachable!(),
            }
            assert!(
                project_conventional_record_with_depth_eleven_nested_sums_materialization_layout(
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
        .data_members(ninth)
        .iter()
        .find_map(|member| match member {
            DataMember::Field(field) if field.name.as_str() == "eighth" => {
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
                symbol: ninth.symbol,
                name: ninth.name.clone(),
            },
        );
    assert!(
        project_conventional_record_with_depth_eleven_nested_sums_materialization_layout(
            &recursive_checked,
            &plan,
            outer.symbol,
        )
        .is_err(),
        "a recursive depth-eleven path must reject during bounded projection"
    );
    assert!(
        validate_const_materializable_record_with_depth_eleven_nested_sums(
            &recursive_checked,
            "Outer",
            &paths,
            &value,
            ByteOrder::LittleEndian,
        )
        .is_err(),
        "a recursive depth-eleven path must reject during value replay"
    );

    assert!(
        project_conventional_record_with_depth_ten_nested_sums_materialization_layout(
            &checked,
            &plan,
            ninth.symbol,
        )
        .is_ok(),
        "the unchanged plural depth-ten API retains its prior cohort"
    );
    assert!(
        project_conventional_record_with_depth_ten_nested_sums_materialization_layout(
            &checked,
            &plan,
            outer.symbol,
        )
        .is_err(),
        "the prior API must not widen to the new depth-eleven root"
    );
}

#[test]
fn plural_depth_twelve_paths_compose_depth_eleven_custody_and_retain_fences() {
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
        data Outer [copy] { #1 left: Tenth; #2 right: Tenth; }

        data OuterTooDeep [copy] { #1 outer: Outer; }
        data OuterDirect [copy] { #1 tenth: Tenth; #2 direct: Choice; }
        data OuterArray [copy] { #1 tenth: Tenth; #2 choices: [Choice; 1]; }
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
    let tenth = definition("Tenth");
    let paths = project_conventional_record_with_depth_twelve_nested_sums_materialization_layout(
        &checked,
        &plan,
        outer.symbol,
    )
    .expect("the complete depth-twelve occurrence cohort should project");
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
    let value = BuildTimeValue::Struct {
        type_name: "Outer".into(),
        fields: vec![
            ("left".into(), tenth_value(choice(None))),
            ("right".into(), tenth_value(choice(Some(0x1122)))),
        ],
    };
    let carrier = validate_const_materializable_record_with_depth_twelve_nested_sums(
        &checked,
        "Outer",
        &paths,
        &value,
        ByteOrder::LittleEndian,
    )
    .expect("depth-twelve value custody should compose the depth-eleven carriers");
    assert_eq!(carrier.occurrences().len(), 2);
    assert_eq!(carrier.occurrences()[0].inner().occurrences().len(), 1);
    assert_eq!(carrier.occurrences()[1].inner().occurrences().len(), 1);
    let mut expected = vec![0; 16];
    expected[8..12].copy_from_slice(&1_u32.to_le_bytes());
    expected[12..14].copy_from_slice(&0x1122_u16.to_le_bytes());
    assert_eq!(carrier.bytes(), expected, "every padding byte remains zero");

    let big_endian = validate_const_materializable_record_with_depth_twelve_nested_sums(
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

    let rejects =
        |mutated: &psi_layout_plans::ConventionalDepthTwelveRecordSumPathsLayoutReport| {
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

    for name in ["Tenth", "OuterTooDeep", "OuterDirect", "OuterArray"] {
        assert!(
            project_conventional_record_with_depth_twelve_nested_sums_materialization_layout(
                &checked,
                &plan,
                definition(name).symbol,
            )
            .is_err(),
            "{name} remains outside the exact depth-twelve cohort"
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
        "Outer", "Tenth", "Ninth", "Eighth", "Seventh", "Sixth", "Fifth", "Fourth", "Third",
        "Second", "First", "Middle", "Leaf",
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
                        interpretation: psi_layout_plans::IntegerInterpretation::Unsigned,
                        write_is_total: true,
                    }),
                _ => unreachable!(),
            }
            assert!(
                project_conventional_record_with_depth_twelve_nested_sums_materialization_layout(
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
        .data_members(tenth)
        .iter()
        .find_map(|member| match member {
            DataMember::Field(field) if field.name.as_str() == "ninth" => {
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
                symbol: tenth.symbol,
                name: tenth.name.clone(),
            },
        );
    assert!(
        project_conventional_record_with_depth_twelve_nested_sums_materialization_layout(
            &recursive_checked,
            &plan,
            outer.symbol,
        )
        .is_err(),
        "a recursive depth-twelve path must reject during bounded projection"
    );
    assert!(
        validate_const_materializable_record_with_depth_twelve_nested_sums(
            &recursive_checked,
            "Outer",
            &paths,
            &value,
            ByteOrder::LittleEndian,
        )
        .is_err(),
        "a recursive depth-twelve path must reject during value replay"
    );

    assert!(
        project_conventional_record_with_depth_eleven_nested_sums_materialization_layout(
            &checked,
            &plan,
            tenth.symbol,
        )
        .is_ok(),
        "the unchanged plural depth-eleven API retains its prior cohort"
    );
    assert!(
        project_conventional_record_with_depth_eleven_nested_sums_materialization_layout(
            &checked,
            &plan,
            outer.symbol,
        )
        .is_err(),
        "the prior API must not widen to the new depth-twelve root"
    );
}

#[test]
fn plural_depth_thirteen_paths_compose_depth_twelve_custody_and_retain_fences() {
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
        data Outer [copy] { #1 left: Eleventh; #2 right: Eleventh; }

        data OuterTooDeep [copy] { #1 outer: Outer; }
        data OuterDirect [copy] { #1 eleventh: Eleventh; #2 direct: Choice; }
        data OuterArray [copy] { #1 eleventh: Eleventh; #2 choices: [Choice; 1]; }
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
    let eleventh = definition("Eleventh");
    let paths = project_conventional_record_with_depth_thirteen_nested_sums_materialization_layout(
        &checked,
        &plan,
        outer.symbol,
    )
    .expect("the complete depth-thirteen occurrence cohort should project");
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
    let value = BuildTimeValue::Struct {
        type_name: "Outer".into(),
        fields: vec![
            ("left".into(), eleventh_value(choice(None))),
            ("right".into(), eleventh_value(choice(Some(0x1122)))),
        ],
    };
    let carrier = psi_build_time_evaluation::validate_const_materializable_record_with_depth_thirteen_nested_sums(
        &checked,
        "Outer",
        &paths,
        &value,
        ByteOrder::LittleEndian,
    )
    .expect("depth-thirteen value custody should compose the depth-twelve carriers");
    assert_eq!(carrier.occurrences().len(), 2);
    assert_eq!(carrier.occurrences()[0].inner().occurrences().len(), 1);
    assert_eq!(carrier.occurrences()[1].inner().occurrences().len(), 1);
    let mut expected = vec![0; 16];
    expected[8..12].copy_from_slice(&1_u32.to_le_bytes());
    expected[12..14].copy_from_slice(&0x1122_u16.to_le_bytes());
    assert_eq!(carrier.bytes(), expected, "every padding byte remains zero");

    let big_endian = psi_build_time_evaluation::validate_const_materializable_record_with_depth_thirteen_nested_sums(
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

    let rejects =
        |mutated: &psi_layout_plans::ConventionalDepthThirteenRecordSumPathsLayoutReport| {
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

    for name in ["Eleventh", "OuterTooDeep", "OuterDirect", "OuterArray"] {
        assert!(
            project_conventional_record_with_depth_thirteen_nested_sums_materialization_layout(
                &checked,
                &plan,
                definition(name).symbol,
            )
            .is_err(),
            "{name} remains outside the exact depth-thirteen cohort"
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
        "Outer", "Eleventh", "Tenth", "Ninth", "Eighth", "Seventh", "Sixth", "Fifth", "Fourth",
        "Third", "Second", "First", "Middle", "Leaf",
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
                        interpretation: psi_layout_plans::IntegerInterpretation::Unsigned,
                        write_is_total: true,
                    }),
                _ => unreachable!(),
            }
            assert!(
                project_conventional_record_with_depth_thirteen_nested_sums_materialization_layout(
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
        .data_members(eleventh)
        .iter()
        .find_map(|member| match member {
            DataMember::Field(field) if field.name.as_str() == "tenth" => {
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
                symbol: eleventh.symbol,
                name: eleventh.name.clone(),
            },
        );
    assert!(
        project_conventional_record_with_depth_thirteen_nested_sums_materialization_layout(
            &recursive_checked,
            &plan,
            outer.symbol,
        )
        .is_err(),
        "a recursive depth-thirteen path must reject during bounded projection"
    );
    assert!(
        psi_build_time_evaluation::validate_const_materializable_record_with_depth_thirteen_nested_sums(
            &recursive_checked,
            "Outer",
            &paths,
            &value,
            ByteOrder::LittleEndian,
        )
        .is_err(),
        "a recursive depth-thirteen path must reject during value replay"
    );

    assert!(
        project_conventional_record_with_depth_twelve_nested_sums_materialization_layout(
            &checked,
            &plan,
            eleventh.symbol,
        )
        .is_ok(),
        "the unchanged plural depth-twelve API retains its prior cohort"
    );
    assert!(
        project_conventional_record_with_depth_twelve_nested_sums_materialization_layout(
            &checked,
            &plan,
            outer.symbol,
        )
        .is_err(),
        "the prior API must not widen to the new depth-thirteen root"
    );
}

#[test]
fn plural_depth_fourteen_paths_compose_depth_thirteen_custody_and_retain_fences() {
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
        data Outer [copy] { #1 left: Twelfth; #2 right: Twelfth; }

        data OuterTooDeep [copy] { #1 outer: Outer; }
        data OuterDirect [copy] { #1 twelfth: Twelfth; #2 direct: Choice; }
        data OuterArray [copy] { #1 twelfth: Twelfth; #2 choices: [Choice; 1]; }
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
    let twelfth = definition("Twelfth");
    let paths = project_conventional_record_with_depth_fourteen_nested_sums_materialization_layout(
        &checked,
        &plan,
        outer.symbol,
    )
    .expect("the complete depth-fourteen occurrence cohort should project");
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
    let value = BuildTimeValue::Struct {
        type_name: "Outer".into(),
        fields: vec![
            ("left".into(), twelfth_value(choice(None))),
            ("right".into(), twelfth_value(choice(Some(0x1122)))),
        ],
    };
    let carrier = psi_build_time_evaluation::validate_const_materializable_record_with_depth_fourteen_nested_sums(
        &checked,
        "Outer",
        &paths,
        &value,
        ByteOrder::LittleEndian,
    )
    .expect("depth-fourteen value custody should compose the depth-thirteen carriers");
    assert_eq!(carrier.occurrences().len(), 2);
    assert_eq!(carrier.occurrences()[0].inner().occurrences().len(), 1);
    assert_eq!(carrier.occurrences()[1].inner().occurrences().len(), 1);
    let mut expected = vec![0; 16];
    expected[8..12].copy_from_slice(&1_u32.to_le_bytes());
    expected[12..14].copy_from_slice(&0x1122_u16.to_le_bytes());
    assert_eq!(carrier.bytes(), expected, "every padding byte remains zero");

    let big_endian = psi_build_time_evaluation::validate_const_materializable_record_with_depth_fourteen_nested_sums(
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

    let rejects =
        |mutated: &psi_layout_plans::ConventionalDepthFourteenRecordSumPathsLayoutReport| {
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

    for name in ["Twelfth", "OuterTooDeep", "OuterDirect", "OuterArray"] {
        assert!(
            project_conventional_record_with_depth_fourteen_nested_sums_materialization_layout(
                &checked,
                &plan,
                definition(name).symbol,
            )
            .is_err(),
            "{name} remains outside the exact depth-fourteen cohort"
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
        "Outer", "Twelfth", "Eleventh", "Tenth", "Ninth", "Eighth", "Seventh", "Sixth", "Fifth",
        "Fourth", "Third", "Second", "First", "Middle", "Leaf",
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
                        interpretation: psi_layout_plans::IntegerInterpretation::Unsigned,
                        write_is_total: true,
                    }),
                _ => unreachable!(),
            }
            assert!(
                project_conventional_record_with_depth_fourteen_nested_sums_materialization_layout(
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
        .data_members(twelfth)
        .iter()
        .find_map(|member| match member {
            DataMember::Field(field) if field.name.as_str() == "eleventh" => {
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
                symbol: twelfth.symbol,
                name: twelfth.name.clone(),
            },
        );
    assert!(
        project_conventional_record_with_depth_fourteen_nested_sums_materialization_layout(
            &recursive_checked,
            &plan,
            outer.symbol,
        )
        .is_err(),
        "a recursive depth-fourteen path must reject during bounded projection"
    );
    assert!(
        psi_build_time_evaluation::validate_const_materializable_record_with_depth_fourteen_nested_sums(
            &recursive_checked,
            "Outer",
            &paths,
            &value,
            ByteOrder::LittleEndian,
        )
        .is_err(),
        "a recursive depth-fourteen path must reject during value replay"
    );

    assert!(
        project_conventional_record_with_depth_thirteen_nested_sums_materialization_layout(
            &checked,
            &plan,
            twelfth.symbol,
        )
        .is_ok(),
        "the unchanged plural depth-thirteen API retains its prior cohort"
    );
    assert!(
        project_conventional_record_with_depth_thirteen_nested_sums_materialization_layout(
            &checked,
            &plan,
            outer.symbol,
        )
        .is_err(),
        "the prior API must not widen to the new depth-fourteen root"
    );
}
#[test]
fn plural_depth_fifteen_paths_compose_depth_fourteen_custody_and_retain_fences() {
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
        data Outer [copy] { #1 left: Thirteenth; #2 right: Thirteenth; }

        data OuterTooDeep [copy] { #1 outer: Outer; }
        data OuterDirect [copy] { #1 thirteenth: Thirteenth; #2 direct: Choice; }
        data OuterArray [copy] { #1 thirteenth: Thirteenth; #2 choices: [Choice; 1]; }
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
    let thirteenth = definition("Thirteenth");
    let paths = project_conventional_record_with_depth_fifteen_nested_sums_materialization_layout(
        &checked,
        &plan,
        outer.symbol,
    )
    .expect("the complete depth-fifteen occurrence cohort should project");
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
    let value = BuildTimeValue::Struct {
        type_name: "Outer".into(),
        fields: vec![
            ("left".into(), thirteenth_value(choice(None))),
            ("right".into(), thirteenth_value(choice(Some(0x1122)))),
        ],
    };
    let carrier = psi_build_time_evaluation::validate_const_materializable_record_with_depth_fifteen_nested_sums(
        &checked,
        "Outer",
        &paths,
        &value,
        ByteOrder::LittleEndian,
    )
    .expect("depth-fifteen value custody should compose the depth-fourteen carriers");
    assert_eq!(carrier.occurrences().len(), 2);
    assert_eq!(carrier.occurrences()[0].inner().occurrences().len(), 1);
    assert_eq!(carrier.occurrences()[1].inner().occurrences().len(), 1);
    let mut expected = vec![0; 16];
    expected[8..12].copy_from_slice(&1_u32.to_le_bytes());
    expected[12..14].copy_from_slice(&0x1122_u16.to_le_bytes());
    assert_eq!(carrier.bytes(), expected, "every padding byte remains zero");

    let big_endian = psi_build_time_evaluation::validate_const_materializable_record_with_depth_fifteen_nested_sums(
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

    let rejects =
        |mutated: &psi_layout_plans::ConventionalDepthFifteenRecordSumPathsLayoutReport| {
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

    for name in ["Thirteenth", "OuterTooDeep", "OuterDirect", "OuterArray"] {
        assert!(
            project_conventional_record_with_depth_fifteen_nested_sums_materialization_layout(
                &checked,
                &plan,
                definition(name).symbol,
            )
            .is_err(),
            "{name} remains outside the exact depth-fifteen cohort"
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
                        interpretation: psi_layout_plans::IntegerInterpretation::Unsigned,
                        write_is_total: true,
                    }),
                _ => unreachable!(),
            }
            assert!(
                project_conventional_record_with_depth_fifteen_nested_sums_materialization_layout(
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
        .data_members(thirteenth)
        .iter()
        .find_map(|member| match member {
            DataMember::Field(field) if field.name.as_str() == "twelfth" => {
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
                symbol: thirteenth.symbol,
                name: thirteenth.name.clone(),
            },
        );
    assert!(
        project_conventional_record_with_depth_fifteen_nested_sums_materialization_layout(
            &recursive_checked,
            &plan,
            outer.symbol,
        )
        .is_err(),
        "a recursive depth-fifteen path must reject during bounded projection"
    );
    assert!(
        psi_build_time_evaluation::validate_const_materializable_record_with_depth_fifteen_nested_sums(
            &recursive_checked,
            "Outer",
            &paths,
            &value,
            ByteOrder::LittleEndian,
        )
        .is_err(),
        "a recursive depth-fifteen path must reject during value replay"
    );

    assert!(
        project_conventional_record_with_depth_fourteen_nested_sums_materialization_layout(
            &checked,
            &plan,
            thirteenth.symbol,
        )
        .is_ok(),
        "the unchanged plural depth-fourteen API retains its prior cohort"
    );
    assert!(
        project_conventional_record_with_depth_fourteen_nested_sums_materialization_layout(
            &checked,
            &plan,
            outer.symbol,
        )
        .is_err(),
        "the prior API must not widen to the new depth-fifteen root"
    );
}
