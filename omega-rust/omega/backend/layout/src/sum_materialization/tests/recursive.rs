//! Recursive record/sum projection and replay, including hostile geometry and custody.
use super::{
    BuildTimeValue, ByteOrder, DataMember, DataShape, LayoutPlacementReport, NativeTarget,
    SymbolHandle, TypeLayoutDescriptor, TypeReferenceNode, checked, unique_data_layout,
};
use crate::project_conventional_record_with_recursive_nested_sums_materialization_layout;
use build_time_evaluation::ValidatedConstRecordWithRecursiveNestedSumsMaterialization;
use build_time_evaluation::validate_const_materializable_record_with_recursive_nested_sums;
use layout_plans::ConventionalRecordSumPathsLayoutReport;
use layout_plans::ConventionalRecursiveRecordSumPathsLayoutReport;
use layout_plans::ConventionalSumFieldLayoutReport;

#[test]
fn recursive_inner_siblings_retain_complete_ordered_custody() {
    let checked = checked(
        "data Choice [copy] { case #1 Empty; case #2 Number(#1 value: u16); }
         data Layer0 [copy] { #1 first: Choice; #2 second: Choice; }
         data Middle [copy] { #1 left: Layer0; #2 right: Layer0; }
         data Outer [copy] { #1 middle: Middle; }",
    );
    let plan = crate::build_layout_plan(&checked, NativeTarget::host(), &[]).unwrap();
    let outer = checked
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Outer")
        .unwrap();
    let paths = project_conventional_record_with_recursive_nested_sums_materialization_layout(
        &checked,
        &plan,
        outer.symbol,
    )
    .unwrap();
    let value = BuildTimeValue::Struct {
        type_name: "Outer".into(),
        fields: vec![(
            "middle".into(),
            BuildTimeValue::Struct {
                type_name: "Middle".into(),
                fields: vec![
                    ("left".into(), recursive_value(1, None)),
                    ("right".into(), recursive_value(1, Some(0x1122))),
                ],
            },
        )],
    };
    let carrier = validate_const_materializable_record_with_recursive_nested_sums(
        &checked,
        "Outer",
        &paths,
        &value,
        ByteOrder::LittleEndian,
    )
    .unwrap();
    let mut expected = [0; 32];
    expected[24..28].copy_from_slice(&1_u32.to_le_bytes());
    expected[28..30].copy_from_slice(&0x1122_u16.to_le_bytes());
    assert_eq!(carrier.bytes(), expected);
    for mutation in 0..4 {
        let mut changed = paths.clone();
        let middle = branch_mut(first_descendant_mut(&mut changed, 1));
        assert_eq!(middle.paths.len(), 2);
        match mutation {
            0 => {
                middle.paths.pop();
            }
            1 => middle.paths.push(middle.paths[0].clone()),
            2 => middle.paths.swap(0, 1),
            3 => middle.paths[1] = middle.paths[0].clone(),
            _ => unreachable!(),
        }
        assert!(
            carrier
                .replay_against(&checked, "Outer", &changed, &value, ByteOrder::LittleEndian)
                .is_err()
        );
        assert!(
            validate_const_materializable_record_with_recursive_nested_sums(
                &checked,
                "Outer",
                &changed,
                &value,
                ByteOrder::LittleEndian,
            )
            .is_err()
        );
    }
}

fn branch_mut(
    report: &mut ConventionalRecursiveRecordSumPathsLayoutReport,
) -> &mut ConventionalRecordSumPathsLayoutReport {
    match report {
        ConventionalRecursiveRecordSumPathsLayoutReport::Branch(branch) => branch,
        _ => panic!("expected recursive record branch"),
    }
}

fn first_descendant_mut(
    mut report: &mut ConventionalRecursiveRecordSumPathsLayoutReport,
    depth: usize,
) -> &mut ConventionalRecursiveRecordSumPathsLayoutReport {
    for _ in 0..depth {
        report = &mut branch_mut(report).paths[0].inner;
    }
    report
}

fn leaf_rows_mut(
    report: &mut ConventionalRecursiveRecordSumPathsLayoutReport,
) -> &mut Vec<ConventionalSumFieldLayoutReport> {
    match report {
        ConventionalRecursiveRecordSumPathsLayoutReport::Leaf {
            child_sum_layouts, ..
        } => child_sum_layouts,
        _ => panic!("expected direct-sum leaf"),
    }
}

fn layout_mut(
    report: &mut ConventionalRecursiveRecordSumPathsLayoutReport,
) -> &mut layout_plans::LayoutPlanReport {
    match report {
        ConventionalRecursiveRecordSumPathsLayoutReport::Leaf { outer_layout, .. } => outer_layout,
        ConventionalRecursiveRecordSumPathsLayoutReport::Branch(branch) => &mut branch.outer_layout,
    }
}

fn numbered_rename(report: &mut ConventionalRecursiveRecordSumPathsLayoutReport) {
    for entry in &mut layout_mut(report).entries {
        entry.field = format!("renamed_{}", entry.field);
    }
    match report {
        ConventionalRecursiveRecordSumPathsLayoutReport::Leaf {
            child_sum_layouts, ..
        } => {
            for child in child_sum_layouts {
                child.field = format!("renamed_{}", child.field);
            }
        }
        ConventionalRecursiveRecordSumPathsLayoutReport::Branch(branch) => {
            for path in &mut branch.paths {
                path.outer_field = format!("renamed_{}", path.outer_field);
                numbered_rename(&mut path.inner);
            }
        }
    }
}

fn recursive_source(depth: usize) -> String {
    let mut source = String::from(
        "data Choice [copy] { case #1 Empty; case #2 Number(#1 value: u16); }
         data Layer0 [copy] { #1 first: Choice; #2 second: Choice; }\n",
    );
    for layer in 1..depth {
        source.push_str(&format!(
            "data Layer{layer} [copy] {{ #1 child: Layer{}; }}\n",
            layer - 1
        ));
    }
    source.push_str(&format!(
        "data Outer [copy] {{ #1 left: Layer{inner}; #2 right: Layer{inner}; }}
         data Deeper [copy] {{ #1 outer: Outer; }}
         data Unequal [copy] {{ #1 left: Layer0; #2 right: Layer{inner}; }}
         data Direct [copy] {{ #1 inner: Layer{inner}; #2 choice: Choice; }}
         data Arrays [copy] {{ #1 inner: Layer{inner}; #2 choices: [Choice; 1]; }}
         data InnerDirect [copy] {{ #1 child: Direct; }}
         data InnerArray [copy] {{ #1 child: Arrays; }}",
        inner = depth - 1,
    ));
    source
}

fn recursive_value(depth: usize, number: Option<i64>) -> BuildTimeValue {
    let empty = || BuildTimeValue::Case {
        variant: "Empty".into(),
        payload: Vec::new(),
    };
    let second = number.map_or_else(empty, |value| BuildTimeValue::Case {
        variant: "Number".into(),
        payload: vec![("value".into(), BuildTimeValue::Int(value))],
    });
    let mut value = BuildTimeValue::Struct {
        type_name: "Layer0".into(),
        fields: vec![("first".into(), empty()), ("second".into(), second)],
    };
    for layer in 1..depth {
        value = BuildTimeValue::Struct {
            type_name: format!("Layer{layer}"),
            fields: vec![("child".into(), value)],
        };
    }
    value
}

#[test]
fn recursive_depths_preserve_bytes_ordered_occurrences_and_atomic_replay() {
    // Cover every retired depth-specific entrypoint and exceed their former ceiling.
    for depth in (1..=24).chain([32, 63]) {
        let checked = checked(&recursive_source(depth));
        let plan = crate::build_layout_plan(&checked, NativeTarget::host(), &[]).unwrap();
        let outer = checked
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == "Outer")
            .unwrap();
        let paths = project_conventional_record_with_recursive_nested_sums_materialization_layout(
            &checked,
            &plan,
            outer.symbol,
        )
        .unwrap_or_else(|error| panic!("depth {depth}: {error:?}"));
        assert_eq!(paths.outer_layout().offsets.as_deref(), Some(&[0, 16][..]));
        assert_eq!(paths.outer_layout().size, Some(32));
        let ConventionalRecursiveRecordSumPathsLayoutReport::Branch(root) = &paths else {
            panic!("outer record is a branch");
        };
        assert_eq!(root.paths.len(), 2);
        assert_eq!(root.paths[0].outer_field, "left");
        assert_eq!(root.paths[0].outer_member_identity, Some(1));
        assert_eq!(root.paths[1].outer_field, "right");
        assert_eq!(root.paths[1].outer_member_identity, Some(2));
        let value = BuildTimeValue::Struct {
            type_name: "Outer".into(),
            fields: vec![
                ("left".into(), recursive_value(depth, None)),
                ("right".into(), recursive_value(depth, Some(0x1122))),
            ],
        };
        let carrier = validate_const_materializable_record_with_recursive_nested_sums(
            &checked,
            "Outer",
            &paths,
            &value,
            ByteOrder::LittleEndian,
        )
        .unwrap_or_else(|error| panic!("depth {depth}: {error:?}"));
        let mut expected = vec![0; 32];
        let mut evidence = &carrier;
        for layer in 0..depth {
            let ValidatedConstRecordWithRecursiveNestedSumsMaterialization::Branch(branch) =
                evidence
            else {
                panic!("depth {depth}: record edge {layer} must retain its occurrence custody");
            };
            assert_eq!(branch.occurrences().len(), if layer == 0 { 2 } else { 1 });
            evidence = branch.occurrences()[0].inner();
        }
        let ValidatedConstRecordWithRecursiveNestedSumsMaterialization::Leaf(leaf) = evidence
        else {
            panic!("depth {depth}: direct sums terminate recursive custody");
        };
        assert_eq!(leaf.nested_sums().len(), 2);
        expected[24..28].copy_from_slice(&1_u32.to_le_bytes());
        expected[28..30].copy_from_slice(&0x1122_u16.to_le_bytes());
        assert_eq!(
            carrier.bytes(),
            expected,
            "depth {depth}, including all padding"
        );
        let big = validate_const_materializable_record_with_recursive_nested_sums(
            &checked,
            "Outer",
            &paths,
            &value,
            ByteOrder::BigEndian,
        )
        .unwrap();
        expected[24..28].copy_from_slice(&1_u32.to_be_bytes());
        expected[28..30].copy_from_slice(&0x1122_u16.to_be_bytes());
        assert_eq!(big.bytes(), expected);
        assert_ne!(
            carrier.non_authoritative_materialization_report_fingerprint(),
            big.non_authoritative_materialization_report_fingerprint()
        );
        let mut destination = [0x5a; 36];
        carrier.apply(&checked, &mut destination).unwrap();
        assert_eq!(&destination[..32], carrier.bytes());
        assert_eq!(&destination[32..], &[0x5a; 4]);
        let mut short = [0x6b; 31];
        assert!(carrier.apply(&checked, &mut short).is_err());
        assert_eq!(short, [0x6b; 31]);

        let mut renamed = paths.clone();
        numbered_rename(&mut renamed);
        carrier
            .replay_against(&checked, "Outer", &renamed, &value, ByteOrder::LittleEndian)
            .expect("numbered member spelling is presentation-only at every layer");

        let rejects = |mutated: &ConventionalRecursiveRecordSumPathsLayoutReport| {
            assert!(
                carrier
                    .replay_against(&checked, "Outer", mutated, &value, ByteOrder::LittleEndian)
                    .is_err(),
                "depth {depth}: mutated report must reject"
            );
        };
        for layer in 0..depth {
            for mutation in 0..7 {
                let mut changed = paths.clone();
                let branch = branch_mut(first_descendant_mut(&mut changed, layer));
                match mutation {
                    0 => {
                        branch.paths.pop();
                    }
                    1 => branch.paths.push(branch.paths[0].clone()),
                    2 => branch.paths[0].outer_member_identity = Some(99),
                    3 => {
                        branch.outer_layout.entries[0].placement =
                            LayoutPlacementReport::At { offset: 4 }
                    }
                    4 => branch.outer_layout.size = Some(64),
                    5 => branch.outer_layout.align = 16,
                    6 => {
                        branch.paths[0].outer_field = "not_a_field".into();
                        branch.paths[0].outer_member_identity = None;
                    }
                    _ => unreachable!(),
                }
                rejects(&changed);
                if matches!(mutation, 0 | 1 | 2 | 6) {
                    assert!(
                        validate_const_materializable_record_with_recursive_nested_sums(
                            &checked,
                            "Outer",
                            &changed,
                            &value,
                            ByteOrder::LittleEndian,
                        )
                        .is_err(),
                        "depth {depth}, layer {layer}, malformed path mutation {mutation}"
                    );
                }
            }
        }
        let mut reordered = paths.clone();
        branch_mut(&mut reordered).paths.swap(0, 1);
        rejects(&reordered);
        let mut duplicate = paths.clone();
        let repeated = branch_mut(&mut duplicate).paths[0].clone();
        branch_mut(&mut duplicate).paths[1] = repeated;
        rejects(&duplicate);
        for mutation in 0..8 {
            let mut changed = paths.clone();
            let leaf = first_descendant_mut(&mut changed, depth);
            match mutation {
                0 => {
                    leaf_rows_mut(leaf).pop();
                }
                1 => {
                    let rows = leaf_rows_mut(leaf);
                    rows.push(rows[0].clone());
                }
                2 => leaf_rows_mut(leaf).swap(0, 1),
                3 => leaf_rows_mut(leaf)[0].member_identity = Some(99),
                4 => leaf_rows_mut(leaf)[1].layout.cases[1].payload_fields[0].offset += 1,
                5 => leaf_rows_mut(leaf)[1].layout.cases[1].ordinal = 7,
                6 => {
                    layout_mut(leaf).entries[0].placement = LayoutPlacementReport::At { offset: 4 }
                }
                7 => {
                    let rows = leaf_rows_mut(leaf);
                    rows[1] = rows[0].clone();
                }
                _ => unreachable!(),
            }
            rejects(&changed);
        }
        let mut wrong_variant = paths.clone();
        wrong_variant = first_descendant_mut(&mut wrong_variant, depth).clone();
        rejects(&wrong_variant);
        assert!(
            carrier
                .replay_against(&checked, "Outer", &paths, &value, ByteOrder::BigEndian)
                .is_err()
        );
        for mutation in 0..3 {
            let mut changed_value = value.clone();
            let BuildTimeValue::Struct { fields, .. } = &mut changed_value else {
                unreachable!()
            };
            match mutation {
                0 => fields.swap(0, 1),
                1 => {
                    fields.pop();
                }
                2 => fields[1].1 = recursive_value(depth, Some(0x3344)),
                _ => unreachable!(),
            }
            assert!(
                carrier
                    .replay_against(
                        &checked,
                        "Outer",
                        &paths,
                        &changed_value,
                        ByteOrder::LittleEndian
                    )
                    .is_err()
            );
        }
    }
}

#[test]
fn recursive_projection_rejects_semantic_and_placement_drift_at_every_layer() {
    for depth in (1..=24).chain([32, 63]) {
        let checked = checked(&recursive_source(depth));
        let plan = crate::build_layout_plan(&checked, NativeTarget::host(), &[]).unwrap();
        let definition = |name: &str| {
            checked
                .data_definitions()
                .iter()
                .find(|definition| definition.name.as_str() == name)
                .unwrap()
        };
        let outer = definition("Outer");
        let paths = project_conventional_record_with_recursive_nested_sums_materialization_layout(
            &checked,
            &plan,
            outer.symbol,
        )
        .unwrap();
        let value = BuildTimeValue::Struct {
            type_name: "Outer".into(),
            fields: vec![
                ("left".into(), recursive_value(depth, None)),
                ("right".into(), recursive_value(depth, Some(0x1122))),
            ],
        };
        let carrier = validate_const_materializable_record_with_recursive_nested_sums(
            &checked,
            "Outer",
            &paths,
            &value,
            ByteOrder::LittleEndian,
        )
        .unwrap();
        for name in ["Direct", "Arrays", "InnerDirect", "InnerArray"] {
            assert!(
                project_conventional_record_with_recursive_nested_sums_materialization_layout(
                    &checked,
                    &plan,
                    definition(name).symbol,
                )
                .is_err(),
                "{name} at depth {depth} must retain its semantic fence"
            );
        }
        // Former shallow/deep/singular-cohort fences are not semantic constraints.
        for name in ["Layer0", "Deeper", "Unequal"] {
            let result =
                project_conventional_record_with_recursive_nested_sums_materialization_layout(
                    &checked,
                    &plan,
                    definition(name).symbol,
                );
            if name == "Deeper" && depth == 63 {
                assert!(
                    result.is_err(),
                    "one additional edge exceeds the resource limit"
                );
            } else {
                result.unwrap_or_else(|error| panic!("{name} at depth {depth}: {error:?}"));
            }
        }
        let owners = std::iter::once("Outer".to_owned())
            .chain((0..depth).map(|layer| format!("Layer{layer}")));
        for owner_name in owners {
            let owner = definition(&owner_name);
            let owner_layout = unique_data_layout(&plan, owner.symbol, &owner_name).unwrap();
            let DataShape::Record { fields } = owner_layout.shape else {
                unreachable!()
            };
            let field_symbol = plan.fields.span_or_empty(fields)[0].symbol;
            for mutation in 0..6 {
                let mut changed_plan = plan.clone();
                match mutation {
                    0 => changed_plan
                        .repeated_fields
                        .push(crate::RepeatedFieldLayout {
                            field: field_symbol,
                            element_stride: 16,
                        }),
                    1 => changed_plan.bit_fields.push(crate::BitFieldLayout {
                        field: field_symbol,
                        fragments: Vec::new(),
                    }),
                    2 => changed_plan
                        .stored_integers
                        .push(crate::StoredIntegerLayout {
                            field: field_symbol,
                            stored_width_bits: 8,
                            interpretation: layout_plans::IntegerInterpretation::Unsigned,
                            write_is_total: true,
                        }),
                    3 => {
                        changed_plan.fields.span_mut_or_empty(fields)[0].type_symbol = outer.symbol
                    }
                    4 => {
                        changed_plan.fields.span_mut_or_empty(fields)[0].type_descriptor =
                            TypeLayoutDescriptor::Named {
                                symbol: outer.symbol,
                                name: outer.name.clone(),
                            }
                    }
                    5 => {
                        let field = &mut changed_plan.fields.span_mut_or_empty(fields)[0];
                        let TypeLayoutDescriptor::Named { name, .. } = &mut field.type_descriptor
                        else {
                            panic!("all fixture fields are nominal");
                        };
                        *name = "wrong_descriptor_name".into();
                    }
                    _ => unreachable!(),
                }
                assert!(
                    project_conventional_record_with_recursive_nested_sums_materialization_layout(
                        &checked,
                        &changed_plan,
                        outer.symbol,
                    )
                    .is_err(),
                    "depth {depth}, {owner_name}, mutation {mutation}"
                );
            }
            let field_type = checked
                .data_members(owner)
                .iter()
                .find_map(|member| match member {
                    DataMember::Field(field) => Some(field.type_reference),
                    _ => None,
                })
                .unwrap();
            for malformed in [false, true] {
                let mut changed_checked = checked.clone();
                changed_checked.typed.type_reference_table.substitute_node(
                    field_type,
                    TypeReferenceNode::Named {
                        symbol: if malformed {
                            SymbolHandle::invalid()
                        } else {
                            owner.symbol
                        },
                        name: owner.name.clone(),
                    },
                );
                assert!(
                    project_conventional_record_with_recursive_nested_sums_materialization_layout(
                        &changed_checked,
                        &plan,
                        outer.symbol,
                    )
                    .is_err(),
                    "cycle/malformed nominal at {owner_name}, depth {depth}"
                );
                assert!(
                    validate_const_materializable_record_with_recursive_nested_sums(
                        &changed_checked,
                        "Outer",
                        &paths,
                        &value,
                        ByteOrder::LittleEndian,
                    )
                    .is_err()
                );
                let mut destination = [0x5a; 36];
                assert!(carrier.apply(&changed_checked, &mut destination).is_err());
                assert_eq!(
                    destination, [0x5a; 36],
                    "failed typed replay must not mutate"
                );
            }
        }
    }
}

#[test]
fn recursive_projection_reports_its_resource_depth_limit() {
    let checked = checked(&recursive_source(64));
    let plan = crate::build_layout_plan(&checked, NativeTarget::host(), &[]).unwrap();
    let outer = checked
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Outer")
        .unwrap();
    let error = project_conventional_record_with_recursive_nested_sums_materialization_layout(
        &checked,
        &plan,
        outer.symbol,
    )
    .expect_err("64 record edges exceed the bounded recursive report resource");
    assert!(format!("{error:?}").contains("64"), "{error:?}");
}

#[test]
fn recursive_unequal_depth_siblings_materialize_under_one_report() {
    let checked = checked(&recursive_source(5));
    let plan = crate::build_layout_plan(&checked, NativeTarget::host(), &[]).unwrap();
    let outer = checked
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Unequal")
        .unwrap();
    let paths = project_conventional_record_with_recursive_nested_sums_materialization_layout(
        &checked,
        &plan,
        outer.symbol,
    )
    .unwrap();
    let value = BuildTimeValue::Struct {
        type_name: "Unequal".into(),
        fields: vec![
            ("left".into(), recursive_value(1, Some(0x1122))),
            ("right".into(), recursive_value(5, Some(0x3344))),
        ],
    };
    let carrier = validate_const_materializable_record_with_recursive_nested_sums(
        &checked,
        "Unequal",
        &paths,
        &value,
        ByteOrder::LittleEndian,
    )
    .unwrap();
    let mut expected = [0; 32];
    expected[8..12].copy_from_slice(&1_u32.to_le_bytes());
    expected[12..14].copy_from_slice(&0x1122_u16.to_le_bytes());
    expected[24..28].copy_from_slice(&1_u32.to_le_bytes());
    expected[28..30].copy_from_slice(&0x3344_u16.to_le_bytes());
    assert_eq!(carrier.bytes(), expected);
    let mut destination = [0x5a; 36];
    carrier.apply(&checked, &mut destination).unwrap();
    assert_eq!(&destination[..32], &expected);
    assert_eq!(&destination[32..], &[0x5a; 4]);
}

#[test]
fn recursive_scalar_siblings_preserve_exact_padding_and_bytes() {
    let checked = checked(
        r#"
        data Choice [copy] { case #1 Empty; case #2 Number(#1 value: u16); }
        data Leaf [copy] {
            #1 lead: u8;
            #2 first: Choice;
            #3 marker: u16;
            #4 second: Choice;
            #5 tail: u8;
        }
        data Middle [copy] { #1 lead: u8; #2 leaf: Leaf; #3 tail: u16; }
        data Outer [copy] { #1 lead: u8; #2 middle: Middle; #3 tail: u16; }

        data OuterTwo [copy] { first: Middle; second: Middle; }
        data MiddleTwo [copy] { first: Leaf; second: Leaf; }
        data OuterMiddleTwo [copy] { middle: MiddleTwo; }
        data OuterShallow [copy] { leaf: Leaf; }
        data Deep [copy] { middle: Middle; }
        data OuterDeep [copy] { deep: Deep; }
        data OuterDirect [copy] { middle: Middle; direct: Choice; }
        data MiddleDirect [copy] { leaf: Leaf; direct: Choice; }
        data OuterMiddleDirect [copy] { middle: MiddleDirect; }
        data OuterArray [copy] { middle: Middle; choices: [Choice; 1]; }
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
    let path = project_conventional_record_with_recursive_nested_sums_materialization_layout(
        &checked,
        &plan,
        outer.symbol,
    )
    .expect("one exact depth-two chain should project compositionally");
    let empty = || BuildTimeValue::Case {
        variant: "Empty".into(),
        payload: Vec::new(),
    };
    let number = |value| BuildTimeValue::Case {
        variant: "Number".into(),
        payload: vec![("value".into(), BuildTimeValue::Int(value))],
    };
    let value = BuildTimeValue::Struct {
        type_name: "Outer".into(),
        fields: vec![
            ("lead".into(), BuildTimeValue::Int(0xdd)),
            (
                "middle".into(),
                BuildTimeValue::Struct {
                    type_name: "Middle".into(),
                    fields: vec![
                        ("lead".into(), BuildTimeValue::Int(0xcc)),
                        (
                            "leaf".into(),
                            BuildTimeValue::Struct {
                                type_name: "Leaf".into(),
                                fields: vec![
                                    ("lead".into(), BuildTimeValue::Int(0xaa)),
                                    ("first".into(), empty()),
                                    ("marker".into(), BuildTimeValue::Int(0x1122)),
                                    ("second".into(), number(0x3344)),
                                    ("tail".into(), BuildTimeValue::Int(0xbb)),
                                ],
                            },
                        ),
                        ("tail".into(), BuildTimeValue::Int(0x5566)),
                    ],
                },
            ),
            ("tail".into(), BuildTimeValue::Int(0x7788)),
        ],
    };
    let carrier = validate_const_materializable_record_with_recursive_nested_sums(
        &checked,
        "Outer",
        &path,
        &value,
        ByteOrder::LittleEndian,
    )
    .expect("the target-produced depth-two report should rejoin value custody");
    assert_eq!(
        carrier.bytes(),
        &[
            0xdd, 0, 0, 0, 0xcc, 0, 0, 0, 0xaa, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x22, 0x11, 0, 0,
            1, 0, 0, 0, 0x44, 0x33, 0, 0, 0xbb, 0, 0, 0, 0x66, 0x55, 0, 0, 0x88, 0x77, 0, 0,
        ]
    );

    let mut destination = [0x5a; 48];
    carrier
        .apply(&checked, &mut destination)
        .expect("the complete outer image should copy atomically");
    assert_eq!(&destination[..44], carrier.bytes());
    assert_eq!(&destination[44..], &[0x5a; 4]);
    let mut short = [0x6b; 43];
    assert!(carrier.apply(&checked, &mut short).is_err());
    assert_eq!(short, [0x6b; 43]);
}
