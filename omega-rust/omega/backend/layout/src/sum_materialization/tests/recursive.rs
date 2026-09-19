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
    let (sums, arrays, record_arrays) = match report {
        ConventionalRecursiveRecordSumPathsLayoutReport::Leaf {
            child_sum_layouts,
            child_sum_array_layouts,
            child_record_array_layouts,
            ..
        } => (
            child_sum_layouts,
            child_sum_array_layouts,
            child_record_array_layouts,
        ),
        ConventionalRecursiveRecordSumPathsLayoutReport::Branch(branch) => {
            for path in &mut branch.paths {
                path.outer_field = format!("renamed_{}", path.outer_field);
            }
            (
                &mut branch.child_sum_layouts,
                &mut branch.child_sum_array_layouts,
                &mut branch.child_record_array_layouts,
            )
        }
    };
    for child in sums {
        child.field = format!("renamed_{}", child.field);
    }
    for child in arrays {
        child.field = format!("renamed_{}", child.field);
    }
    for child in record_arrays {
        child.field = format!("renamed_{}", child.field);
        numbered_rename(&mut child.inner);
    }
    if let ConventionalRecursiveRecordSumPathsLayoutReport::Branch(branch) = report {
        for path in &mut branch.paths {
            numbered_rename(&mut path.inner);
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
         data InnerArray [copy] {{ #1 child: Arrays; }}
         data RecordArrays [copy] {{ #1 inner: Layer{inner}; #2 neighbors: [Layer0; 1]; }}
         data NestedChoiceArray [copy] {{ #1 inner: Layer{inner}; #2 matrix: [[Choice; 1]; 1]; }}
         data ZeroChoices [copy] {{ #1 inner: Layer{inner}; #2 none: [Choice; 0]; }}
         data InnerRecordArrays [copy] {{ #1 child: RecordArrays; }}",
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
        // Zero-length array hops stay fenced at every depth. Nested literal
        // arrays flatten into one packed row under the general recursive
        // rule, so `NestedChoiceArray` joins the admitted cohort below.
        {
            let name = "ZeroChoices";
            assert!(
                project_conventional_record_with_recursive_nested_sums_materialization_layout(
                    &checked,
                    &plan,
                    definition(name).symbol,
                )
                .is_err(),
                "{name} at depth {depth} must retain its unsupported-shape fence"
            );
        }
        // Former shallow/deep/singular-cohort fences are not semantic
        // constraints: direct sums, direct sum arrays, direct record arrays,
        // and nested record paths coexist at one level under the general
        // recursive rule.
        for name in [
            "Layer0",
            "Deeper",
            "Unequal",
            "Direct",
            "InnerDirect",
            "Arrays",
            "InnerArray",
            "RecordArrays",
            "InnerRecordArrays",
            "NestedChoiceArray",
        ] {
            let result =
                project_conventional_record_with_recursive_nested_sums_materialization_layout(
                    &checked,
                    &plan,
                    definition(name).symbol,
                );
            if matches!(
                name,
                "Deeper" | "InnerDirect" | "InnerArray" | "InnerRecordArrays"
            ) && depth == 63
            {
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
fn recursive_direct_sums_coexist_with_deeper_paths_on_one_level() {
    // `Direct { inner: LayerN, choice: Choice }` is the lifted coexistence
    // shape: one `Branch` carries its own direct conventional sum beside the
    // deeper record path, and `InnerDirect` nests that level one edge deeper.
    let checked = checked(&recursive_source(2));
    let plan = crate::build_layout_plan(&checked, NativeTarget::host(), &[]).unwrap();
    let definition = |name: &str| {
        checked
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == name)
            .unwrap()
    };
    let paths = project_conventional_record_with_recursive_nested_sums_materialization_layout(
        &checked,
        &plan,
        definition("Direct").symbol,
    )
    .expect("a record holding a direct sum beside a deeper sum path projects");
    let ConventionalRecursiveRecordSumPathsLayoutReport::Branch(root) = &paths else {
        panic!("the coexisting record projects as a recursive branch");
    };
    assert_eq!(root.paths.len(), 1);
    assert_eq!(root.paths[0].outer_field, "inner");
    assert_eq!(root.paths[0].outer_member_identity, Some(1));
    assert_eq!(root.child_sum_layouts.len(), 1);
    assert_eq!(root.child_sum_layouts[0].field, "choice");
    assert_eq!(root.child_sum_layouts[0].member_identity, Some(2));
    assert_eq!(
        paths
            .outer_layout()
            .entries
            .iter()
            .map(|entry| (entry.field.as_str(), entry.placement))
            .collect::<Vec<_>>(),
        vec![
            ("inner", LayoutPlacementReport::At { offset: 0 }),
            ("choice", LayoutPlacementReport::At { offset: 16 }),
        ]
    );

    let value = BuildTimeValue::Struct {
        type_name: "Direct".into(),
        fields: vec![
            ("inner".into(), recursive_value(2, Some(0x1122))),
            (
                "choice".into(),
                BuildTimeValue::Case {
                    variant: "Number".into(),
                    payload: vec![("value".into(), BuildTimeValue::Int(0x5566))],
                },
            ),
        ],
    };
    let carrier = validate_const_materializable_record_with_recursive_nested_sums(
        &checked,
        "Direct",
        &paths,
        &value,
        ByteOrder::LittleEndian,
    )
    .expect("coexisting direct sums retain value custody beside the deeper path");
    let ValidatedConstRecordWithRecursiveNestedSumsMaterialization::Branch(branch) = &carrier
    else {
        panic!("the coexisting record retains branch custody");
    };
    assert_eq!(branch.occurrences().len(), 1);
    assert_eq!(branch.nested_sums().len(), 1);
    assert_eq!(branch.nested_sums()[0].field(), "choice");
    let mut expected = [0; 24];
    expected[8..12].copy_from_slice(&1_u32.to_le_bytes());
    expected[12..14].copy_from_slice(&0x1122_u16.to_le_bytes());
    expected[16..20].copy_from_slice(&1_u32.to_le_bytes());
    expected[20..22].copy_from_slice(&0x5566_u16.to_le_bytes());
    assert_eq!(carrier.bytes(), expected);
    let mut destination = [0x5a; 28];
    carrier.apply(&checked, &mut destination).unwrap();
    assert_eq!(&destination[..24], &expected);
    assert_eq!(&destination[24..], &[0x5a; 4]);

    // The deeper level also admits the coexisting shape one edge down.
    let inner_paths =
        project_conventional_record_with_recursive_nested_sums_materialization_layout(
            &checked,
            &plan,
            definition("InnerDirect").symbol,
        )
        .expect("a record nesting a coexisting level projects recursively");
    let inner_value = BuildTimeValue::Struct {
        type_name: "InnerDirect".into(),
        fields: vec![("child".into(), value.clone())],
    };
    let inner_carrier = validate_const_materializable_record_with_recursive_nested_sums(
        &checked,
        "InnerDirect",
        &inner_paths,
        &inner_value,
        ByteOrder::LittleEndian,
    )
    .expect("the nested coexisting level retains custody");
    assert_eq!(inner_carrier.bytes(), expected);

    // Report drift on the branch level's direct-sum rows rejects on replay
    // and on fresh validation, exactly like the path rows.
    let rejects = |mutated: &ConventionalRecursiveRecordSumPathsLayoutReport| {
        assert!(
            carrier
                .replay_against(&checked, "Direct", mutated, &value, ByteOrder::LittleEndian)
                .is_err(),
            "mutated coexisting report must reject"
        );
        assert!(
            validate_const_materializable_record_with_recursive_nested_sums(
                &checked,
                "Direct",
                mutated,
                &value,
                ByteOrder::LittleEndian,
            )
            .is_err(),
            "mutated coexisting report must not revalidate"
        );
    };
    for mutation in 0..4 {
        let mut changed = paths.clone();
        let branch = branch_mut(&mut changed);
        match mutation {
            0 => {
                branch.child_sum_layouts.pop();
            }
            1 => branch
                .child_sum_layouts
                .push(branch.child_sum_layouts[0].clone()),
            2 => branch.child_sum_layouts[0].member_identity = Some(99),
            3 => {
                branch.child_sum_layouts[0].field = "not_a_field".into();
                branch.child_sum_layouts[0].member_identity = None;
            }
            _ => unreachable!(),
        }
        rejects(&changed);
    }
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

#[test]
fn recursive_sum_arrays_compose_beside_direct_sums_and_deeper_paths() {
    // `PickLeaf` ends the recursion on an array row alone, `BothLeaf` keeps a
    // direct sum beside it, `BothBranch` carries all three child kinds, and
    // `InnerArray` reaches `Arrays` one record edge down. The same level rule
    // admits every one of them.
    let checked = checked(
        &(recursive_source(2)
            + "data PickLeaf [copy] { #1 picks: [Choice; 2]; #2 tail: u16; }
               data BothLeaf [copy] { #1 choice: Choice; #2 picks: [Choice; 2]; }
               data BothBranch [copy] { #1 inner: Layer1; #2 choice: Choice; #3 picks: [Choice; 2]; }"),
    );
    let plan = crate::build_layout_plan(&checked, NativeTarget::host(), &[]).unwrap();
    let definition = |name: &str| {
        checked
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == name)
            .unwrap()
    };
    let number = |value| BuildTimeValue::Case {
        variant: "Number".into(),
        payload: vec![("value".into(), BuildTimeValue::Int(value))],
    };
    let empty = || BuildTimeValue::Case {
        variant: "Empty".into(),
        payload: Vec::new(),
    };

    // Leaf level: one compact array row, no direct sum, no record path.
    let pick_paths = project_conventional_record_with_recursive_nested_sums_materialization_layout(
        &checked,
        &plan,
        definition("PickLeaf").symbol,
    )
    .expect("a leaf level carrying a direct sum array projects");
    let ConventionalRecursiveRecordSumPathsLayoutReport::Leaf {
        outer_layout,
        child_sum_layouts,
        child_sum_array_layouts,
        child_record_array_layouts,
    } = &pick_paths
    else {
        panic!("a record ending on a direct sum array is a leaf level");
    };
    assert!(child_sum_layouts.is_empty());
    assert!(child_record_array_layouts.is_empty());
    assert_eq!(
        child_sum_array_layouts
            .iter()
            .map(|row| (
                row.field.as_str(),
                row.member_identity,
                row.element_count,
                row.element_stride
            ))
            .collect::<Vec<_>>(),
        [("picks", Some(1), 2, 8)]
    );
    assert_eq!(outer_layout.offsets.as_deref(), Some(&[0, 16][..]));
    let pick_value = BuildTimeValue::Struct {
        type_name: "PickLeaf".into(),
        fields: vec![
            (
                "picks".into(),
                BuildTimeValue::Array(vec![number(0x1122), empty()]),
            ),
            ("tail".into(), BuildTimeValue::Int(0x3344)),
        ],
    };
    let carrier = validate_const_materializable_record_with_recursive_nested_sums(
        &checked,
        "PickLeaf",
        &pick_paths,
        &pick_value,
        ByteOrder::LittleEndian,
    )
    .expect("the leaf's array row rejoins value custody");
    let ValidatedConstRecordWithRecursiveNestedSumsMaterialization::Leaf(leaf) = &carrier else {
        panic!("the leaf level retains leaf custody");
    };
    assert!(leaf.nested_sums().is_empty());
    assert_eq!(leaf.nested_sum_arrays().len(), 1);
    let picks = &leaf.nested_sum_arrays()[0];
    assert_eq!(picks.field(), "picks");
    assert_eq!(picks.field_identity(), Some(1));
    assert_eq!(picks.elements().len(), 2);
    assert_eq!(picks.elements()[0].literal_index(), 0);
    assert_eq!(picks.elements()[0].selected_case_ordinal(), 1);
    assert_eq!(picks.elements()[1].literal_index(), 1);
    assert_eq!(picks.elements()[1].selected_case_ordinal(), 0);
    assert_eq!(
        carrier.bytes(),
        &[
            1, 0, 0, 0, 0x22, 0x11, 0, 0, // picks[0] = Number(0x1122)
            0, 0, 0, 0, 0, 0, 0, 0, // picks[1] = Empty
            0x44, 0x33, 0, 0, // tail
        ]
    );
    // The leaf level's own `apply` replays its retained array rows before the
    // one atomic copy.
    let mut leaf_destination = [0x5a; 24];
    carrier.apply(&checked, &mut leaf_destination).unwrap();
    assert_eq!(&leaf_destination[..20], carrier.bytes());
    assert_eq!(&leaf_destination[20..], &[0x5a; 4]);
    let mut leaf_short = [0x6b; 19];
    assert!(carrier.apply(&checked, &mut leaf_short).is_err());
    assert_eq!(leaf_short, [0x6b; 19]);

    // Leaf level: a direct sum and a direct sum array coexist.
    let both_leaf_paths =
        project_conventional_record_with_recursive_nested_sums_materialization_layout(
            &checked,
            &plan,
            definition("BothLeaf").symbol,
        )
        .expect("a leaf level carries a direct sum beside its sum array");
    let ConventionalRecursiveRecordSumPathsLayoutReport::Leaf {
        child_sum_layouts: both_leaf_sums,
        child_sum_array_layouts: both_leaf_arrays,
        ..
    } = &both_leaf_paths
    else {
        panic!("the coexisting leaf stays a leaf level");
    };
    assert_eq!(
        both_leaf_sums
            .iter()
            .map(|row| (row.field.as_str(), row.member_identity))
            .collect::<Vec<_>>(),
        [("choice", Some(1))]
    );
    assert_eq!(
        both_leaf_arrays
            .iter()
            .map(|row| (row.field.as_str(), row.member_identity))
            .collect::<Vec<_>>(),
        [("picks", Some(2))]
    );

    // Branch level: the deeper record path, the level's own direct sum, and
    // its sum array all retain authored-order rows on one report.
    let both_paths = project_conventional_record_with_recursive_nested_sums_materialization_layout(
        &checked,
        &plan,
        definition("BothBranch").symbol,
    )
    .expect("a branch level carries all three child kinds");
    let ConventionalRecursiveRecordSumPathsLayoutReport::Branch(both_root) = &both_paths else {
        panic!("the coexisting record projects as a recursive branch");
    };
    assert_eq!(both_root.paths.len(), 1);
    assert_eq!(both_root.paths[0].outer_field, "inner");
    assert_eq!(both_root.paths[0].outer_member_identity, Some(1));
    assert_eq!(
        both_root
            .child_sum_layouts
            .iter()
            .map(|row| (row.field.as_str(), row.member_identity))
            .collect::<Vec<_>>(),
        [("choice", Some(2))]
    );
    assert_eq!(
        both_root
            .child_sum_array_layouts
            .iter()
            .map(|row| (
                row.field.as_str(),
                row.member_identity,
                row.element_count,
                row.element_stride
            ))
            .collect::<Vec<_>>(),
        [("picks", Some(3), 2, 8)]
    );
    assert_eq!(
        both_paths.outer_layout().offsets.as_deref(),
        Some(&[0, 16, 24][..])
    );
    let both_value = BuildTimeValue::Struct {
        type_name: "BothBranch".into(),
        fields: vec![
            ("inner".into(), recursive_value(2, Some(0x5566))),
            ("choice".into(), number(0x7788)),
            (
                "picks".into(),
                BuildTimeValue::Array(vec![empty(), number(0x1122)]),
            ),
        ],
    };
    let carrier = validate_const_materializable_record_with_recursive_nested_sums(
        &checked,
        "BothBranch",
        &both_paths,
        &both_value,
        ByteOrder::LittleEndian,
    )
    .expect("the branch level's three child kinds rejoin value custody");
    let ValidatedConstRecordWithRecursiveNestedSumsMaterialization::Branch(branch) = &carrier
    else {
        panic!("the coexisting record retains branch custody");
    };
    assert_eq!(branch.occurrences().len(), 1);
    assert_eq!(branch.nested_sums().len(), 1);
    assert_eq!(branch.nested_sums()[0].field(), "choice");
    assert_eq!(branch.nested_sum_arrays().len(), 1);
    assert_eq!(branch.nested_sum_arrays()[0].field(), "picks");
    assert_eq!(branch.nested_sum_arrays()[0].elements().len(), 2);
    assert_eq!(
        carrier.bytes(),
        &[
            0, 0, 0, 0, 0, 0, 0, 0, // inner.child.first = Empty
            1, 0, 0, 0, 0x66, 0x55, 0, 0, // inner.child.second = Number(0x5566)
            1, 0, 0, 0, 0x88, 0x77, 0, 0, // choice = Number(0x7788)
            0, 0, 0, 0, 0, 0, 0, 0, // picks[0] = Empty
            1, 0, 0, 0, 0x22, 0x11, 0, 0, // picks[1] = Number(0x1122)
        ]
    );
    let mut destination = [0x5a; 44];
    carrier.apply(&checked, &mut destination).unwrap();
    assert_eq!(&destination[..40], carrier.bytes());
    assert_eq!(&destination[40..], &[0x5a; 4]);

    // `Arrays` reaches a direct sum array one record edge above its deeper
    // path, and `InnerArray` reaches `Arrays` one edge further down.
    let arrays_paths =
        project_conventional_record_with_recursive_nested_sums_materialization_layout(
            &checked,
            &plan,
            definition("Arrays").symbol,
        )
        .expect("the former top-level-only array shape projects as a branch row");
    let ConventionalRecursiveRecordSumPathsLayoutReport::Branch(arrays_root) = &arrays_paths else {
        panic!("a record path beside a direct sum array is a branch level");
    };
    assert_eq!(arrays_root.paths.len(), 1);
    assert!(arrays_root.child_sum_layouts.is_empty());
    assert_eq!(arrays_root.child_sum_array_layouts.len(), 1);
    assert_eq!(arrays_root.child_sum_array_layouts[0].field, "choices");
    let arrays_value = BuildTimeValue::Struct {
        type_name: "Arrays".into(),
        fields: vec![
            ("inner".into(), recursive_value(2, Some(0x3344))),
            (
                "choices".into(),
                BuildTimeValue::Array(vec![number(0x1122)]),
            ),
        ],
    };
    let arrays_carrier = validate_const_materializable_record_with_recursive_nested_sums(
        &checked,
        "Arrays",
        &arrays_paths,
        &arrays_value,
        ByteOrder::LittleEndian,
    )
    .expect("the nested sum array retains custody at the branch level");
    assert_eq!(
        arrays_carrier.bytes(),
        &[
            0, 0, 0, 0, 0, 0, 0, 0, // inner.child.first = Empty
            1, 0, 0, 0, 0x44, 0x33, 0, 0, // inner.child.second = Number(0x3344)
            1, 0, 0, 0, 0x22, 0x11, 0, 0, // choices[0] = Number(0x1122)
        ]
    );
    let inner_paths =
        project_conventional_record_with_recursive_nested_sums_materialization_layout(
            &checked,
            &plan,
            definition("InnerArray").symbol,
        )
        .expect("a record reaching a sum array one edge down projects recursively");
    let inner_value = BuildTimeValue::Struct {
        type_name: "InnerArray".into(),
        fields: vec![("child".into(), arrays_value.clone())],
    };
    let inner_carrier = validate_const_materializable_record_with_recursive_nested_sums(
        &checked,
        "InnerArray",
        &inner_paths,
        &inner_value,
        ByteOrder::LittleEndian,
    )
    .expect("the nested sum-array level retains custody");
    assert_eq!(inner_carrier.bytes(), arrays_carrier.bytes());

    // Drift on a retained array row rejects on replay and on fresh
    // validation, exactly like the direct-sum and path rows.
    let rejects = |mutated: &ConventionalRecursiveRecordSumPathsLayoutReport| {
        assert!(
            carrier
                .replay_against(
                    &checked,
                    "BothBranch",
                    mutated,
                    &both_value,
                    ByteOrder::LittleEndian
                )
                .is_err(),
            "mutated array report must reject"
        );
        assert!(
            validate_const_materializable_record_with_recursive_nested_sums(
                &checked,
                "BothBranch",
                mutated,
                &both_value,
                ByteOrder::LittleEndian,
            )
            .is_err(),
            "mutated array report must not revalidate"
        );
    };
    for mutation in 0..6 {
        let mut changed = both_paths.clone();
        let branch = branch_mut(&mut changed);
        match mutation {
            0 => {
                branch.child_sum_array_layouts.pop();
            }
            1 => branch
                .child_sum_array_layouts
                .push(branch.child_sum_array_layouts[0].clone()),
            2 => branch.child_sum_array_layouts[0].member_identity = Some(99),
            3 => {
                branch.child_sum_array_layouts[0].field = "not_a_field".into();
                branch.child_sum_array_layouts[0].member_identity = None;
            }
            4 => branch.child_sum_array_layouts[0].element_count = 3,
            5 => branch.child_sum_array_layouts[0].element_stride += 8,
            _ => unreachable!(),
        }
        rejects(&changed);
    }

    // Numbered member spelling stays presentation-only on the array row too.
    let mut renamed = both_paths.clone();
    numbered_rename(&mut renamed);
    carrier
        .replay_against(
            &checked,
            "BothBranch",
            &renamed,
            &both_value,
            ByteOrder::LittleEndian,
        )
        .expect("numbered member spelling is presentation-only on array rows");
}

#[test]
fn recursive_record_arrays_compose_beside_direct_sums_and_deeper_paths() {
    // `NeighborLeaf` ends the recursion on record-array rows alone,
    // `RecordArrays` keeps a deeper record path beside its array, and
    // `InnerRecordArrays` reaches that level one record edge down. The same
    // general level rule admits every one of them: each row retains the
    // literal count and stride plus the element record's shared recursive
    // report, and every index carries its own recursive custody.
    let checked = checked(
        &(recursive_source(2)
            + "data NeighborLeaf [copy] { #1 neighbors: [Layer0; 2]; #2 tail: u16; }"),
    );
    let plan = crate::build_layout_plan(&checked, NativeTarget::host(), &[]).unwrap();
    let definition = |name: &str| {
        checked
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == name)
            .unwrap()
    };
    let number = |value| BuildTimeValue::Case {
        variant: "Number".into(),
        payload: vec![("value".into(), BuildTimeValue::Int(value))],
    };
    let empty = || BuildTimeValue::Case {
        variant: "Empty".into(),
        payload: Vec::new(),
    };
    let layer0 = |first, second| BuildTimeValue::Struct {
        type_name: "Layer0".into(),
        fields: vec![("first".into(), first), ("second".into(), second)],
    };

    // Leaf level: one compact record-array row, no direct sum, no record
    // path — a record ending on `[R; N]` alone still ends the recursion.
    let leaf_paths = project_conventional_record_with_recursive_nested_sums_materialization_layout(
        &checked,
        &plan,
        definition("NeighborLeaf").symbol,
    )
    .expect("a leaf level carrying a direct record array projects");
    let ConventionalRecursiveRecordSumPathsLayoutReport::Leaf {
        outer_layout,
        child_sum_layouts,
        child_sum_array_layouts,
        child_record_array_layouts,
    } = &leaf_paths
    else {
        panic!("a record ending on a direct record array is a leaf level");
    };
    assert!(child_sum_layouts.is_empty());
    assert!(child_sum_array_layouts.is_empty());
    assert_eq!(
        child_record_array_layouts
            .iter()
            .map(|row| (
                row.field.as_str(),
                row.member_identity,
                row.element_count,
                row.element_stride
            ))
            .collect::<Vec<_>>(),
        [("neighbors", Some(1), 2, 16)]
    );
    // The shared element report is the element record's own leaf level.
    let ConventionalRecursiveRecordSumPathsLayoutReport::Leaf {
        outer_layout: element_layout,
        child_sum_layouts: element_sums,
        child_sum_array_layouts: element_arrays,
        child_record_array_layouts: element_record_arrays,
    } = &child_record_array_layouts[0].inner
    else {
        panic!("the Layer0 element is a leaf level");
    };
    assert_eq!(element_layout.size, Some(16));
    assert_eq!(element_layout.offsets.as_deref(), Some(&[0, 8][..]));
    assert!(element_arrays.is_empty());
    assert!(element_record_arrays.is_empty());
    assert_eq!(
        element_sums
            .iter()
            .map(|row| (row.field.as_str(), row.member_identity))
            .collect::<Vec<_>>(),
        [("first", Some(1)), ("second", Some(2))]
    );
    assert_eq!(outer_layout.offsets.as_deref(), Some(&[0, 32][..]));

    let leaf_value = BuildTimeValue::Struct {
        type_name: "NeighborLeaf".into(),
        fields: vec![
            (
                "neighbors".into(),
                BuildTimeValue::Array(vec![
                    layer0(number(0x1122), empty()),
                    layer0(empty(), number(0x3344)),
                ]),
            ),
            ("tail".into(), BuildTimeValue::Int(0x5566)),
        ],
    };
    let carrier = validate_const_materializable_record_with_recursive_nested_sums(
        &checked,
        "NeighborLeaf",
        &leaf_paths,
        &leaf_value,
        ByteOrder::LittleEndian,
    )
    .expect("the leaf's record-array row rejoins value custody");
    let ValidatedConstRecordWithRecursiveNestedSumsMaterialization::Leaf(leaf) = &carrier else {
        panic!("the leaf level retains leaf custody");
    };
    assert!(leaf.nested_sums().is_empty());
    assert!(leaf.nested_sum_arrays().is_empty());
    assert_eq!(leaf.nested_record_arrays().len(), 1);
    let neighbors = &leaf.nested_record_arrays()[0];
    assert_eq!(neighbors.field(), "neighbors");
    assert_eq!(neighbors.field_identity(), Some(1));
    assert_eq!(neighbors.elements().len(), 2);
    assert_eq!(neighbors.elements()[0].literal_index(), 0);
    assert_eq!(neighbors.elements()[1].literal_index(), 1);
    assert_eq!(
        carrier.bytes(),
        &[
            1, 0, 0, 0, 0x22, 0x11, 0, 0, // neighbors[0].first = Number(0x1122)
            0, 0, 0, 0, 0, 0, 0, 0, // neighbors[0].second = Empty
            0, 0, 0, 0, 0, 0, 0, 0, // neighbors[1].first = Empty
            1, 0, 0, 0, 0x44, 0x33, 0, 0, // neighbors[1].second = Number(0x3344)
            0x66, 0x55, 0, 0, // tail
        ]
    );
    // The leaf level's own `apply` replays its retained record-array rows
    // before the one atomic copy.
    let mut leaf_destination = [0x5a; 40];
    carrier.apply(&checked, &mut leaf_destination).unwrap();
    assert_eq!(&leaf_destination[..36], carrier.bytes());
    assert_eq!(&leaf_destination[36..], &[0x5a; 4]);
    let mut leaf_short = [0x6b; 35];
    assert!(carrier.apply(&checked, &mut leaf_short).is_err());
    assert_eq!(leaf_short, [0x6b; 35]);

    // Branch level: the deeper record path and the record array retain
    // authored-order rows on one report.
    let record_paths =
        project_conventional_record_with_recursive_nested_sums_materialization_layout(
            &checked,
            &plan,
            definition("RecordArrays").symbol,
        )
        .expect("a branch level carries a record path beside its record array");
    let ConventionalRecursiveRecordSumPathsLayoutReport::Branch(record_root) = &record_paths else {
        panic!("a record path beside a direct record array is a branch level");
    };
    assert_eq!(record_root.paths.len(), 1);
    assert_eq!(record_root.paths[0].outer_field, "inner");
    assert_eq!(record_root.paths[0].outer_member_identity, Some(1));
    assert!(record_root.child_sum_layouts.is_empty());
    assert!(record_root.child_sum_array_layouts.is_empty());
    assert_eq!(
        record_root
            .child_record_array_layouts
            .iter()
            .map(|row| (
                row.field.as_str(),
                row.member_identity,
                row.element_count,
                row.element_stride
            ))
            .collect::<Vec<_>>(),
        [("neighbors", Some(2), 1, 16)]
    );
    let record_value = BuildTimeValue::Struct {
        type_name: "RecordArrays".into(),
        fields: vec![
            ("inner".into(), recursive_value(2, Some(0x5566))),
            (
                "neighbors".into(),
                BuildTimeValue::Array(vec![layer0(number(0x7788), empty())]),
            ),
        ],
    };
    let carrier = validate_const_materializable_record_with_recursive_nested_sums(
        &checked,
        "RecordArrays",
        &record_paths,
        &record_value,
        ByteOrder::LittleEndian,
    )
    .expect("the branch level's record-array row rejoins value custody");
    let ValidatedConstRecordWithRecursiveNestedSumsMaterialization::Branch(branch) = &carrier
    else {
        panic!("the coexisting record retains branch custody");
    };
    assert_eq!(branch.occurrences().len(), 1);
    assert!(branch.nested_sums().is_empty());
    assert!(branch.nested_sum_arrays().is_empty());
    assert_eq!(branch.nested_record_arrays().len(), 1);
    assert_eq!(branch.nested_record_arrays()[0].field(), "neighbors");
    assert_eq!(branch.nested_record_arrays()[0].elements().len(), 1);
    assert_eq!(
        carrier.bytes(),
        &[
            0, 0, 0, 0, 0, 0, 0, 0, // inner.child.first = Empty
            1, 0, 0, 0, 0x66, 0x55, 0, 0, // inner.child.second = Number(0x5566)
            1, 0, 0, 0, 0x88, 0x77, 0, 0, // neighbors[0].first = Number(0x7788)
            0, 0, 0, 0, 0, 0, 0, 0, // neighbors[0].second = Empty
        ]
    );
    let mut destination = [0x5a; 36];
    carrier.apply(&checked, &mut destination).unwrap();
    assert_eq!(&destination[..32], carrier.bytes());
    assert_eq!(&destination[32..], &[0x5a; 4]);

    // `InnerRecordArrays` reaches the record-array level one edge deeper.
    let inner_paths =
        project_conventional_record_with_recursive_nested_sums_materialization_layout(
            &checked,
            &plan,
            definition("InnerRecordArrays").symbol,
        )
        .expect("a record reaching a record array one edge down projects recursively");
    let inner_value = BuildTimeValue::Struct {
        type_name: "InnerRecordArrays".into(),
        fields: vec![("child".into(), record_value.clone())],
    };
    let inner_carrier = validate_const_materializable_record_with_recursive_nested_sums(
        &checked,
        "InnerRecordArrays",
        &inner_paths,
        &inner_value,
        ByteOrder::LittleEndian,
    )
    .expect("the nested record-array level retains custody");
    assert_eq!(inner_carrier.bytes(), carrier.bytes());

    // Drift on a retained record-array row — identity, count, stride, or the
    // shared element report — rejects on replay and on fresh validation,
    // exactly like the direct-sum, sum-array, and path rows.
    let rejects = |mutated: &ConventionalRecursiveRecordSumPathsLayoutReport| {
        assert!(
            carrier
                .replay_against(
                    &checked,
                    "RecordArrays",
                    mutated,
                    &record_value,
                    ByteOrder::LittleEndian
                )
                .is_err(),
            "mutated record-array report must reject"
        );
        assert!(
            validate_const_materializable_record_with_recursive_nested_sums(
                &checked,
                "RecordArrays",
                mutated,
                &record_value,
                ByteOrder::LittleEndian,
            )
            .is_err(),
            "mutated record-array report must not revalidate"
        );
    };
    for mutation in 0..7 {
        let mut changed = record_paths.clone();
        let branch = branch_mut(&mut changed);
        match mutation {
            0 => {
                branch.child_record_array_layouts.pop();
            }
            1 => branch
                .child_record_array_layouts
                .push(branch.child_record_array_layouts[0].clone()),
            2 => branch.child_record_array_layouts[0].member_identity = Some(99),
            3 => {
                branch.child_record_array_layouts[0].field = "not_a_field".into();
                branch.child_record_array_layouts[0].member_identity = None;
            }
            4 => branch.child_record_array_layouts[0].element_count = 3,
            5 => branch.child_record_array_layouts[0].element_stride += 8,
            6 => {
                leaf_rows_mut(&mut branch.child_record_array_layouts[0].inner).pop();
            }
            _ => unreachable!(),
        }
        rejects(&changed);
    }

    // Value drift inside one indexed element rejects replay and validation.
    for mutation in 0..3 {
        let mut changed_value = record_value.clone();
        let BuildTimeValue::Struct { fields, .. } = &mut changed_value else {
            unreachable!()
        };
        let BuildTimeValue::Array(elements) = &mut fields[1].1 else {
            unreachable!()
        };
        match mutation {
            0 => {
                elements.pop();
            }
            1 => elements.push(layer0(empty(), empty())),
            2 => elements[0] = recursive_value(2, None),
            _ => unreachable!(),
        }
        assert!(
            validate_const_materializable_record_with_recursive_nested_sums(
                &checked,
                "RecordArrays",
                &record_paths,
                &changed_value,
                ByteOrder::LittleEndian,
            )
            .is_err(),
            "mutation {mutation}: malformed record-array value must not validate"
        );
    }

    // Numbered member spelling stays presentation-only on the record-array
    // row and inside its shared element report.
    let mut renamed = record_paths.clone();
    numbered_rename(&mut renamed);
    carrier
        .replay_against(
            &checked,
            "RecordArrays",
            &renamed,
            &record_value,
            ByteOrder::LittleEndian,
        )
        .expect("numbered member spelling is presentation-only on record-array rows");
}

#[test]
fn recursive_nested_literal_arrays_flatten_into_packed_rows() {
    // `[[Choice; 2]; 3]` is six packed sums byte-identical to `[Choice; 6]`:
    // one compact row carries the hop product at the innermost element's
    // stride, the flat leaf index `matrix[k]` spells `k = outer * 2 + inner`,
    // and every declared level's arity is still enforced on the value.
    // `[[Layer0; 2]; 2]` flattens the same way with the record element's own
    // leaf report retained once beside the packed row. Zero-length hops and
    // values whose per-level arity drifts stay fenced.
    let checked = checked(
        "data Choice [copy] { case #1 Empty; case #2 Number(#1 value: u16); }
         data Layer0 [copy] { #1 first: Choice; #2 second: Choice; }
         data Packed [copy] { #1 head: u16; #2 matrix: [[Choice; 2]; 3]; #3 rows: [[Layer0; 2]; 2]; #4 tail: Choice; }
         data ZeroNested [copy] { #1 dead: [[Choice; 0]; 2]; }",
    );
    let plan = crate::build_layout_plan(&checked, NativeTarget::host(), &[]).unwrap();
    let definition = |name: &str| {
        checked
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == name)
            .unwrap()
    };
    let zero_error = project_conventional_record_with_recursive_nested_sums_materialization_layout(
        &checked,
        &plan,
        definition("ZeroNested").symbol,
    )
    .expect_err("a zero-length nested hop must reject");
    assert!(
        format!("{zero_error:?}").contains("must have nonzero literal length"),
        "{zero_error:?}"
    );

    let paths = project_conventional_record_with_recursive_nested_sums_materialization_layout(
        &checked,
        &plan,
        definition("Packed").symbol,
    )
    .expect("a level carrying nested literal arrays beside a direct sum projects");
    let ConventionalRecursiveRecordSumPathsLayoutReport::Leaf {
        outer_layout,
        child_sum_layouts,
        child_sum_array_layouts,
        child_record_array_layouts,
    } = &paths
    else {
        panic!("Packed holds no record path: the level is a leaf");
    };
    assert_eq!(outer_layout.offsets.as_deref(), Some(&[0, 4, 52, 116][..]));
    assert_eq!(outer_layout.size, Some(124));
    assert_eq!(
        child_sum_layouts
            .iter()
            .map(|row| (row.field.as_str(), row.member_identity))
            .collect::<Vec<_>>(),
        [("tail", Some(4))]
    );
    assert_eq!(
        child_sum_array_layouts
            .iter()
            .map(|row| (
                row.field.as_str(),
                row.member_identity,
                row.element_count,
                row.element_stride
            ))
            .collect::<Vec<_>>(),
        [("matrix", Some(2), 6, 8)]
    );
    assert_eq!(
        child_record_array_layouts
            .iter()
            .map(|row| (
                row.field.as_str(),
                row.member_identity,
                row.element_count,
                row.element_stride
            ))
            .collect::<Vec<_>>(),
        [("rows", Some(3), 4, 16)]
    );
    // The packed record-array row retains the element's leaf level once —
    // Layer0's own two direct sums — not four multiplied rows.
    let ConventionalRecursiveRecordSumPathsLayoutReport::Leaf {
        child_sum_layouts: element_sums,
        child_sum_array_layouts: element_arrays,
        child_record_array_layouts: element_record_arrays,
        ..
    } = &child_record_array_layouts[0].inner
    else {
        panic!("the Layer0 element is a leaf level");
    };
    assert_eq!(
        element_sums
            .iter()
            .map(|row| (row.field.as_str(), row.member_identity))
            .collect::<Vec<_>>(),
        [("first", Some(1)), ("second", Some(2))]
    );
    assert!(element_arrays.is_empty());
    assert!(element_record_arrays.is_empty());

    let number = |value| BuildTimeValue::Case {
        variant: "Number".into(),
        payload: vec![("value".into(), BuildTimeValue::Int(value))],
    };
    let empty = || BuildTimeValue::Case {
        variant: "Empty".into(),
        payload: Vec::new(),
    };
    let layer0 = |first, second| BuildTimeValue::Struct {
        type_name: "Layer0".into(),
        fields: vec![("first".into(), first), ("second".into(), second)],
    };
    let value = BuildTimeValue::Struct {
        type_name: "Packed".into(),
        fields: vec![
            ("head".into(), BuildTimeValue::Int(0x7788)),
            (
                "matrix".into(),
                BuildTimeValue::Array(vec![
                    BuildTimeValue::Array(vec![empty(), number(0x1122)]),
                    BuildTimeValue::Array(vec![number(0x3344), empty()]),
                    BuildTimeValue::Array(vec![number(0x5566), empty()]),
                ]),
            ),
            (
                "rows".into(),
                BuildTimeValue::Array(vec![
                    BuildTimeValue::Array(vec![
                        layer0(number(0x7788), empty()),
                        layer0(empty(), number(0x99aa)),
                    ]),
                    BuildTimeValue::Array(vec![
                        layer0(empty(), empty()),
                        layer0(number(0xbbcc), empty()),
                    ]),
                ]),
            ),
            ("tail".into(), number(0xddee)),
        ],
    };
    let carrier = validate_const_materializable_record_with_recursive_nested_sums(
        &checked,
        "Packed",
        &paths,
        &value,
        ByteOrder::LittleEndian,
    )
    .expect("nested literal arrays rejoin value custody as packed rows");
    let ValidatedConstRecordWithRecursiveNestedSumsMaterialization::Leaf(leaf) = &carrier else {
        panic!("Packed retains leaf custody");
    };
    assert_eq!(leaf.nested_sums().len(), 1);
    assert_eq!(leaf.nested_sums()[0].field(), "tail");
    assert_eq!(leaf.nested_sum_arrays().len(), 1);
    let matrix = &leaf.nested_sum_arrays()[0];
    assert_eq!(matrix.field(), "matrix");
    assert_eq!(matrix.field_identity(), Some(2));
    // `literal_index` is the packed leaf index: `matrix[1][0]` is element 2.
    assert_eq!(
        matrix
            .elements()
            .iter()
            .map(|element| element.literal_index())
            .collect::<Vec<_>>(),
        [0, 1, 2, 3, 4, 5]
    );
    assert_eq!(leaf.nested_record_arrays().len(), 1);
    let rows = &leaf.nested_record_arrays()[0];
    assert_eq!(rows.field(), "rows");
    assert_eq!(
        rows.elements()
            .iter()
            .map(|element| element.literal_index())
            .collect::<Vec<_>>(),
        [0, 1, 2, 3]
    );
    let mut expected = [0_u8; 124];
    expected[0..2].copy_from_slice(&0x7788_u16.to_le_bytes());
    // Packed index 1 is `matrix[0][1]`, index 2 is `matrix[1][0]`, index 4 is
    // `matrix[2][0]` — the flat leaf order proves `k = outer * 2 + inner`.
    for (flat_index, case) in [(1_usize, 0x1122_u16), (2, 0x3344), (4, 0x5566)] {
        let offset = 4 + flat_index * 8;
        expected[offset..offset + 4].copy_from_slice(&1_u32.to_le_bytes());
        expected[offset + 4..offset + 6].copy_from_slice(&case.to_le_bytes());
    }
    expected[52..56].copy_from_slice(&1_u32.to_le_bytes());
    expected[56..58].copy_from_slice(&0x7788_u16.to_le_bytes());
    expected[76..80].copy_from_slice(&1_u32.to_le_bytes());
    expected[80..82].copy_from_slice(&0x99aa_u16.to_le_bytes());
    expected[100..104].copy_from_slice(&1_u32.to_le_bytes());
    expected[104..106].copy_from_slice(&0xbbcc_u16.to_le_bytes());
    expected[116..120].copy_from_slice(&1_u32.to_le_bytes());
    expected[120..122].copy_from_slice(&0xddee_u16.to_le_bytes());
    assert_eq!(carrier.bytes(), &expected);
    let mut destination = [0x5a; 128];
    carrier.apply(&checked, &mut destination).unwrap();
    assert_eq!(&destination[..124], carrier.bytes());
    assert_eq!(&destination[124..], &[0x5a; 4]);

    // Per-level arity is part of the packed contract: the same total count
    // spelled at the wrong level, or a value missing one level entirely,
    // rejects against the declared hops.
    for mutation in 0..3 {
        let mut changed_value = value.clone();
        let BuildTimeValue::Struct { fields, .. } = &mut changed_value else {
            unreachable!()
        };
        match mutation {
            0 => {
                let BuildTimeValue::Array(outer) = &mut fields[1].1 else {
                    unreachable!()
                };
                outer.pop();
            }
            1 => {
                let BuildTimeValue::Array(outer) = &mut fields[1].1 else {
                    unreachable!()
                };
                let BuildTimeValue::Array(inner) = &mut outer[1] else {
                    unreachable!()
                };
                inner.pop();
            }
            2 => {
                fields[1].1 = BuildTimeValue::Array(vec![
                    empty(),
                    empty(),
                    empty(),
                    empty(),
                    empty(),
                    empty(),
                ]);
            }
            _ => unreachable!(),
        }
        assert!(
            validate_const_materializable_record_with_recursive_nested_sums(
                &checked,
                "Packed",
                &paths,
                &changed_value,
                ByteOrder::LittleEndian,
            )
            .is_err(),
            "mutation {mutation}: per-level arity drift must reject"
        );
    }
}

#[test]
fn symbolic_length_spelling_fences_the_owner_until_checking_substitutes_it() {
    // The crate-local `checked()` path skips the orchestration const-eval
    // pass, so a machine-call array length stays `ConstCall` here — exactly
    // the shape the non-literal-length fence exists for. On the connected
    // pipeline the same program arrives already substituted (the compiler
    // test spells `Neighbor<two()>` into the closed `Neighbor<2>` instance).
    let checked = checked(
        r#"
        machine two() -> u64 { 2 }
        data Choice [copy] { case Empty; case Number(value: u8); }
        data CallLen [copy] { items: [Choice; two()]; }
        "#,
    );
    let plan = crate::build_layout_plan(&checked, NativeTarget::host(), &[]).unwrap();
    let owner = checked
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "CallLen")
        .unwrap();
    let error = project_conventional_record_with_recursive_nested_sums_materialization_layout(
        &checked,
        &plan,
        owner.symbol,
    )
    .expect_err("an unsubstituted machine-call length stays fenced");
    assert!(
        error
            .message
            .contains("reaches a sum through a non-literal-length array"),
        "{error:?}"
    );
}
