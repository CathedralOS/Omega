use super::{
    ConventionalNestedRecordSumPathLayoutReport, DataMember, DataShape, LayoutPlacementReport,
    SumReachability, SymbolHandle, TypeLayoutDescriptor, TypeReferenceNode,
    project_conventional_record_with_nested_sum_record_materialization_layout,
    project_conventional_record_with_nested_sum_records_materialization_layout,
    project_conventional_record_with_sum_array_materialization_layout,
    project_conventional_record_with_sum_arrays_materialization_layout,
    project_conventional_record_with_sum_materialization_layout,
    project_conventional_sum_materialization_layout, unique_data_layout,
};
use build_time_evaluation::{
    BuildTimeValue, validate_const_materializable_conventional_sum,
    validate_const_materializable_record_with_conventional_sum,
    validate_const_materializable_record_with_conventional_sum_array,
    validate_const_materializable_record_with_conventional_sum_arrays,
    validate_const_materializable_record_with_nested_sum_record,
    validate_const_materializable_record_with_nested_sum_records,
};
use checked_trees::{CheckFacts, CheckedTrees};
use layout_plans::{ByteOrder, normalized_conventional_sum_layout_report_fingerprint};
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
use target::NativeTarget;
use tokens_to_syntax_trees::parse_syntax_trees;

mod recursive;

fn checked(source: &str) -> CheckedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    CheckedTrees::with_roots(typed, CheckFacts::default())
}
#[test]
fn projects_exact_authored_case_order_and_overlay_geometry() {
    let checked = checked(
        r#"
        data Choice [copy] {
            case Empty;
            case Number(value: u8, proof [erased]: u64);
            case Pair(left: u16, right: u32);
        }
        "#,
    );
    let definition = checked
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Choice")
        .unwrap();
    let plan = crate::build_layout_plan(&checked, NativeTarget::host(), &[]).unwrap();
    let report =
        project_conventional_sum_materialization_layout(&checked, &plan, definition.symbol)
            .unwrap();

    assert_eq!(report.tag_offset, 0);
    assert_eq!(report.tag_size, 4);
    assert_eq!(report.tag_align, 4);
    assert_eq!(report.size, 12);
    assert_eq!(report.align, 4);
    assert_eq!(
        report
            .cases
            .iter()
            .map(|case| (case.case.as_str(), case.ordinal))
            .collect::<Vec<_>>(),
        [("Empty", 0), ("Number", 1), ("Pair", 2)]
    );
    assert!(report.cases[0].payload_fields.is_empty());
    assert_eq!(report.cases[1].payload_fields[0].offset, 4);
    assert_eq!(
        report.cases[2]
            .payload_fields
            .iter()
            .map(|field| (field.field.as_str(), field.offset, field.size))
            .collect::<Vec<_>>(),
        [("left", 4, 2), ("right", 8, 4)]
    );
    assert_ne!(
        normalized_conventional_sum_layout_report_fingerprint(&report),
        0
    );

    let value = BuildTimeValue::Case {
        variant: "Pair".into(),
        payload: vec![
            ("left".into(), BuildTimeValue::Int(0x1122)),
            ("right".into(), BuildTimeValue::Int(0x3344_5566)),
        ],
    };
    let materialized = validate_const_materializable_conventional_sum(
        &checked,
        "Choice",
        &report,
        &value,
        ByteOrder::LittleEndian,
    )
    .expect("authoritative runtime report should materialize its active case");
    assert_eq!(
        materialized.bytes(),
        &[2, 0, 0, 0, 0x22, 0x11, 0, 0, 0x66, 0x55, 0x44, 0x33]
    );
}

#[test]
fn mixed_common_field_shape_projects_common_rows_beside_the_case_overlay() {
    let checked = checked(
        r#"
        data Event [copy] {
            sequence: u8;
            case Ready(value: u16);
            case Waiting;
        }
        "#,
    );
    let definition = checked
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Event")
        .unwrap();
    let plan = crate::build_layout_plan(&checked, NativeTarget::host(), &[]).unwrap();
    let report =
        project_conventional_sum_materialization_layout(&checked, &plan, definition.symbol)
            .expect("a mixed common-field/case shape projects under the same rung");

    // The tag leads at 0; `sequence` packs at 4; the shared payload base
    // aligns to 6 under Ready.value's u16 alignment, so Ready.value sits at
    // 6 and the 8-byte extent covers common plus payload.
    assert_eq!(report.tag_offset, 0);
    assert_eq!(report.tag_size, 4);
    assert_eq!(report.size, 8);
    assert_eq!(report.align, 4);
    assert_eq!(
        report
            .common_fields
            .iter()
            .map(|field| (field.field.as_str(), field.offset, field.size, field.align))
            .collect::<Vec<_>>(),
        [("sequence", 4, 1, 1)]
    );
    assert_eq!(
        report
            .cases
            .iter()
            .map(|case| (case.case.as_str(), case.ordinal))
            .collect::<Vec<_>>(),
        [("Ready", 0), ("Waiting", 1)]
    );
    assert_eq!(
        report.cases[0]
            .payload_fields
            .iter()
            .map(|field| (field.field.as_str(), field.offset, field.size))
            .collect::<Vec<_>>(),
        [("value", 6, 2)]
    );

    // The projected report is authoritative: a merged value materializes the
    // common field and the selected case payload at their exact offsets.
    let value = BuildTimeValue::Case {
        variant: "Ready".into(),
        payload: vec![
            ("sequence".into(), BuildTimeValue::Int(9)),
            ("value".into(), BuildTimeValue::Int(0x1122)),
        ],
    };
    let materialized = validate_const_materializable_conventional_sum(
        &checked,
        "Event",
        &report,
        &value,
        ByteOrder::LittleEndian,
    )
    .expect("authoritative mixed report should materialize its active case");
    assert_eq!(materialized.bytes(), &[0, 0, 0, 0, 9, 0, 0x22, 0x11]);
}

#[test]
fn target_path_projects_and_replays_one_inner_record_with_complete_direct_sums() {
    let checked = checked(
        r#"
        data Choice [copy] { case #1 Empty; case #2 Number(#1 value: u8); }
        data Inner [copy] { #1 first: Choice; #2 marker: u16; #3 second: Choice; }
        data Outer [copy] { #1 prefix: u8; #2 inner: Inner; #3 suffix: u16; }
        "#,
    );
    let outer = checked
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Outer")
        .unwrap();
    let inner = checked
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Inner")
        .unwrap();
    let plan = crate::build_layout_plan(&checked, NativeTarget::host(), &[]).unwrap();
    let path = project_conventional_record_with_nested_sum_record_materialization_layout(
        &checked,
        &plan,
        outer.symbol,
    )
    .expect("one exact target path should project");
    assert_eq!(path.outer_field, "inner");
    assert_eq!(path.outer_member_identity, Some(2));
    assert_eq!(path.outer_layout.offsets.as_deref(), Some(&[0, 4, 24][..]));
    assert_eq!(path.inner_layout.offsets.as_deref(), Some(&[0, 8, 12][..]));
    assert_eq!(
        path.child_sum_layouts
            .iter()
            .map(|row| row.field.as_str())
            .collect::<Vec<_>>(),
        ["first", "second"]
    );

    let value = BuildTimeValue::Struct {
        type_name: "Outer".into(),
        fields: vec![
            ("prefix".into(), BuildTimeValue::Int(0xaa)),
            (
                "inner".into(),
                BuildTimeValue::Struct {
                    type_name: "Inner".into(),
                    fields: vec![
                        (
                            "first".into(),
                            BuildTimeValue::Case {
                                variant: "Empty".into(),
                                payload: Vec::new(),
                            },
                        ),
                        ("marker".into(), BuildTimeValue::Int(0x1122)),
                        (
                            "second".into(),
                            BuildTimeValue::Case {
                                variant: "Number".into(),
                                payload: vec![("value".into(), BuildTimeValue::Int(0x5c))],
                            },
                        ),
                    ],
                },
            ),
            ("suffix".into(), BuildTimeValue::Int(0x3344)),
        ],
    };
    let carrier = validate_const_materializable_record_with_nested_sum_record(
        &checked,
        "Outer",
        &path,
        &value,
        ByteOrder::LittleEndian,
    )
    .expect("target path should feed exact nested-record materialization");
    assert_eq!(carrier.inner().nested_sums().len(), 2);
    assert_eq!(
        carrier.inner().nested_sums()[0]
            .nested_sum()
            .selected_case_ordinal(),
        0
    );
    assert_eq!(
        carrier.inner().nested_sums()[1]
            .nested_sum()
            .selected_case_ordinal(),
        1
    );
    assert_eq!(
        carrier.bytes(),
        &[
            0xaa, 0, 0, 0, // outer prefix padding
            0, 0, 0, 0, 0, 0, 0, 0, // first Empty
            0x22, 0x11, 0, 0, // marker plus inner padding
            1, 0, 0, 0, 0x5c, 0, 0, 0, // second Number
            0x44, 0x33, 0, 0, // outer suffix and padding
        ]
    );
    carrier
        .replay_against(&checked, "Outer", &path, &value, ByteOrder::LittleEndian)
        .expect("the complete path should replay exactly");

    let mut renamed_reports = path.clone();
    renamed_reports.outer_field = "renamed_inner".into();
    renamed_reports.outer_layout.entries[1].field = "renamed_inner".into();
    renamed_reports.inner_layout.entries[0].field = "renamed_first".into();
    renamed_reports.child_sum_layouts[0].field = "renamed_first".into();
    carrier
        .replay_against(
            &checked,
            "Outer",
            &renamed_reports,
            &value,
            ByteOrder::LittleEndian,
        )
        .expect("stable-numbered report names are presentation-only");

    let mut wrong_outer_identity = path.clone();
    wrong_outer_identity.outer_member_identity = path.outer_layout.entries[0].member_identity;
    assert!(
        carrier
            .replay_against(
                &checked,
                "Outer",
                &wrong_outer_identity,
                &value,
                ByteOrder::LittleEndian,
            )
            .is_err()
    );
    let mut wrong_inner_layout = path.clone();
    wrong_inner_layout.inner_layout.entries[1].placement = LayoutPlacementReport::At { offset: 10 };
    assert!(
        carrier
            .replay_against(
                &checked,
                "Outer",
                &wrong_inner_layout,
                &value,
                ByteOrder::LittleEndian,
            )
            .is_err()
    );
    let mut missing_child = path.clone();
    missing_child.child_sum_layouts.pop();
    assert!(
        carrier
            .replay_against(
                &checked,
                "Outer",
                &missing_child,
                &value,
                ByteOrder::LittleEndian,
            )
            .is_err()
    );
    let mut extra_child = path.clone();
    extra_child
        .child_sum_layouts
        .push(path.child_sum_layouts[0].clone());
    assert!(
        carrier
            .replay_against(
                &checked,
                "Outer",
                &extra_child,
                &value,
                ByteOrder::LittleEndian,
            )
            .is_err()
    );
    let mut duplicate_child = path.clone();
    duplicate_child.child_sum_layouts[1] = path.child_sum_layouts[0].clone();
    assert!(
        carrier
            .replay_against(
                &checked,
                "Outer",
                &duplicate_child,
                &value,
                ByteOrder::LittleEndian,
            )
            .is_err()
    );
    let mut wrong_child_identity = path.clone();
    wrong_child_identity.child_sum_layouts[0].member_identity =
        path.child_sum_layouts[1].member_identity;
    assert!(
        carrier
            .replay_against(
                &checked,
                "Outer",
                &wrong_child_identity,
                &value,
                ByteOrder::LittleEndian,
            )
            .is_err()
    );
    let mut reordered_children = path.clone();
    reordered_children.child_sum_layouts.swap(0, 1);
    assert!(
        carrier
            .replay_against(
                &checked,
                "Outer",
                &reordered_children,
                &value,
                ByteOrder::LittleEndian,
            )
            .is_err()
    );
    let mut wrong_child_geometry = path.clone();
    wrong_child_geometry.child_sum_layouts[1].layout.cases[1].payload_fields[0].offset += 1;
    assert!(
        carrier
            .replay_against(
                &checked,
                "Outer",
                &wrong_child_geometry,
                &value,
                ByteOrder::LittleEndian,
            )
            .is_err()
    );
    let mut short = [0xa5; 27];
    assert!(carrier.apply(&checked, &mut short).is_err());
    assert_eq!(short, [0xa5; 27]);

    let inner_layout = unique_data_layout(&plan, inner.symbol, "Inner").unwrap();
    let DataShape::Record {
        fields: inner_fields,
    } = inner_layout.shape
    else {
        unreachable!("fixture inner is a record")
    };
    let mut substituted_child_plan = plan.clone();
    substituted_child_plan
        .fields
        .span_mut_or_empty(inner_fields)[0]
        .type_descriptor = TypeLayoutDescriptor::Unit;
    assert!(
        project_conventional_record_with_nested_sum_record_materialization_layout(
            &checked,
            &substituted_child_plan,
            outer.symbol,
        )
        .is_err(),
        "a child sum descriptor substitution must reject"
    );

    let outer_data_layout = unique_data_layout(&plan, outer.symbol, "Outer").unwrap();
    let DataShape::Record {
        fields: outer_fields,
    } = outer_data_layout.shape
    else {
        unreachable!("fixture outer is a record")
    };
    let mut substituted_outer_plan = plan.clone();
    substituted_outer_plan
        .fields
        .span_mut_or_empty(outer_fields)[1]
        .type_descriptor = TypeLayoutDescriptor::Unit;
    assert!(
        project_conventional_record_with_nested_sum_record_materialization_layout(
            &checked,
            &substituted_outer_plan,
            outer.symbol,
        )
        .is_err(),
        "the exact outer-to-inner descriptor must rejoin"
    );

    let outer_field_symbol = plan.fields.span_or_empty(outer_fields)[0].symbol;
    let mut repeated_outer_plan = plan.clone();
    repeated_outer_plan
        .repeated_fields
        .push(crate::RepeatedFieldLayout {
            field: outer_field_symbol,
            element_stride: 1,
        });
    assert!(
        project_conventional_record_with_nested_sum_record_materialization_layout(
            &checked,
            &repeated_outer_plan,
            outer.symbol,
        )
        .is_err(),
        "target-dependent placement on any outer field must reject"
    );
    let inner_field_symbol = plan.fields.span_or_empty(inner_fields)[1].symbol;
    let mut repeated_inner_plan = plan.clone();
    repeated_inner_plan
        .repeated_fields
        .push(crate::RepeatedFieldLayout {
            field: inner_field_symbol,
            element_stride: 2,
        });
    assert!(
        project_conventional_record_with_nested_sum_record_materialization_layout(
            &checked,
            &repeated_inner_plan,
            outer.symbol,
        )
        .is_err(),
        "target-dependent placement on any inner field must reject"
    );
}

#[test]
fn nested_record_path_projection_fences_competing_and_deeper_sum_shapes() {
    let checked = checked(
        r#"
        data Choice [copy] { case Empty; case Number(value: u8); }
        data Inner [copy] { choice: Choice; }
        data Deep [copy] { inner: Inner; }
        data ArrayInner [copy] { choices: [Choice; 2]; }
        data DirectOuter [copy] { inner: Inner; direct: Choice; }
        data TwoInner [copy] { first: Inner; second: Inner; }
        data ArrayChild [copy] { inner: Inner; array: [Choice; 1]; }
        data DeeperChild [copy] { inner: Inner; deeper: Deep; }
        data OuterArraySibling [copy] { inner: Inner; sibling: [Choice; 1]; }
        "#,
    );
    let plan = crate::build_layout_plan(&checked, NativeTarget::host(), &[]).unwrap();
    for name in [
        "DirectOuter",
        "TwoInner",
        "ArrayChild",
        "DeeperChild",
        "OuterArraySibling",
    ] {
        let definition = checked
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == name)
            .unwrap();
        assert!(
            project_conventional_record_with_nested_sum_record_materialization_layout(
                &checked,
                &plan,
                definition.symbol,
            )
            .is_err(),
            "{name} must remain outside the singular one-level path cohort"
        );
    }

    let outer = checked
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "ArrayInner")
        .unwrap();
    assert!(
        project_conventional_record_with_nested_sum_record_materialization_layout(
            &checked,
            &plan,
            outer.symbol,
        )
        .is_err()
    );
}

#[test]
fn plural_nested_record_paths_retain_complete_ordered_occurrences_and_replay_atomically() {
    let checked = checked(
        r#"
        data Choice [copy] { case #1 Empty; case #2 Number(#1 value: u8); }
        data Inner [copy] {
            #1 choice: Choice;
            #2 marker: u16;
            #3 backup: Choice;
        }
        data Outer [copy] {
            #1 prefix: u8;
            #2 first: Inner;
            #3 between: u16;
            #4 second: Inner;
            #5 suffix: u8;
        }
        "#,
    );
    let outer = checked
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Outer")
        .unwrap();
    let plan = crate::build_layout_plan(&checked, NativeTarget::host(), &[]).unwrap();
    let paths = project_conventional_record_with_nested_sum_records_materialization_layout(
        &checked,
        &plan,
        outer.symbol,
    )
    .expect("the complete authored-order path set should project");
    assert_eq!(
        paths.outer_layout.offsets.as_deref(),
        Some(&[0, 4, 24, 28, 48][..])
    );
    assert_eq!(paths.outer_layout.size, Some(52));
    assert_eq!(paths.paths.len(), 2);
    assert_eq!(paths.paths[0].outer_field, "first");
    assert_eq!(paths.paths[1].outer_field, "second");
    assert_eq!(paths.paths[0].outer_member_identity, Some(2));
    assert_eq!(paths.paths[1].outer_member_identity, Some(4));
    assert_eq!(paths.paths[0].inner_layout, paths.paths[1].inner_layout);
    assert_eq!(paths.paths[0].child_sum_layouts.len(), 2);
    assert_eq!(paths.paths[1].child_sum_layouts.len(), 2);
    assert!(
        project_conventional_record_with_nested_sum_record_materialization_layout(
            &checked,
            &plan,
            outer.symbol,
        )
        .is_err(),
        "the singular compatibility projection must fail closed on two occurrences"
    );

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
            ("prefix".into(), BuildTimeValue::Int(0xaa)),
            (
                "first".into(),
                BuildTimeValue::Struct {
                    type_name: "Inner".into(),
                    fields: vec![
                        ("choice".into(), empty()),
                        ("marker".into(), BuildTimeValue::Int(0x1122)),
                        ("backup".into(), number(0x3a)),
                    ],
                },
            ),
            ("between".into(), BuildTimeValue::Int(0x3344)),
            (
                "second".into(),
                BuildTimeValue::Struct {
                    type_name: "Inner".into(),
                    fields: vec![
                        ("choice".into(), number(0x5c)),
                        ("marker".into(), BuildTimeValue::Int(0x5566)),
                        ("backup".into(), empty()),
                    ],
                },
            ),
            ("suffix".into(), BuildTimeValue::Int(0x77)),
        ],
    };
    let singular_first = ConventionalNestedRecordSumPathLayoutReport {
        outer_layout: paths.outer_layout.clone(),
        outer_field: paths.paths[0].outer_field.clone(),
        outer_member_identity: paths.paths[0].outer_member_identity,
        inner_layout: paths.paths[0].inner_layout.clone(),
        child_sum_layouts: paths.paths[0].child_sum_layouts.clone(),
    };
    assert!(
        validate_const_materializable_record_with_nested_sum_record(
            &checked,
            "Outer",
            &singular_first,
            &value,
            ByteOrder::LittleEndian,
        )
        .is_err(),
        "the singular consumer must not discard the second qualifying occurrence"
    );
    let carrier = validate_const_materializable_record_with_nested_sum_records(
        &checked,
        "Outer",
        &paths,
        &value,
        ByteOrder::LittleEndian,
    )
    .expect("two same-type occurrences should retain independent selected sums");
    assert_eq!(carrier.inner_records().len(), 2);
    assert_eq!(
        carrier.inner_records()[0].inner().nested_sums()[0]
            .nested_sum()
            .selected_case_ordinal(),
        0
    );
    assert_eq!(
        carrier.inner_records()[1].inner().nested_sums()[0]
            .nested_sum()
            .selected_case_ordinal(),
        1
    );
    assert_eq!(
        carrier.bytes(),
        &[
            0xaa, 0, 0, 0, // prefix padding
            0, 0, 0, 0, 0, 0, 0, 0, // first.choice Empty
            0x22, 0x11, 0, 0, // first.marker + padding
            1, 0, 0, 0, 0x3a, 0, 0, 0, // first.backup Number
            0x44, 0x33, 0, 0, // between + outer padding
            1, 0, 0, 0, 0x5c, 0, 0, 0, // second.choice Number
            0x66, 0x55, 0, 0, // second.marker + padding
            0, 0, 0, 0, 0, 0, 0, 0, // second.backup Empty
            0x77, 0, 0, 0, // suffix + tail padding
        ]
    );
    carrier
        .replay_against(&checked, "Outer", &paths, &value, ByteOrder::LittleEndian)
        .expect("the complete plural path report should replay");
    let mut destination = [0xa5; 56];
    carrier
        .apply(&checked, &mut destination)
        .expect("all occurrences replay before one outer copy");
    assert_eq!(&destination[..52], carrier.bytes());
    assert_eq!(&destination[52..], &[0xa5; 4]);
    let mut short = [0x5a; 51];
    assert!(carrier.apply(&checked, &mut short).is_err());
    assert_eq!(short, [0x5a; 51]);

    let mut renamed = paths.clone();
    renamed.outer_layout.entries[1].field = "renamed_first".into();
    renamed.outer_layout.entries[3].field = "renamed_second".into();
    renamed.paths[0].outer_field = "renamed_first".into();
    renamed.paths[1].outer_field = "renamed_second".into();
    renamed.paths[0].inner_layout.entries[0].field = "renamed_choice".into();
    renamed.paths[0].child_sum_layouts[0].field = "renamed_choice".into();
    carrier
        .replay_against(&checked, "Outer", &renamed, &value, ByteOrder::LittleEndian)
        .expect("stable-numbered outer and child names remain presentation-only");

    let rejects = |mutated: &layout_plans::ConventionalNestedRecordSumPathsLayoutReport| {
        assert!(
            carrier
                .replay_against(&checked, "Outer", mutated, &value, ByteOrder::LittleEndian,)
                .is_err()
        );
    };
    let mut missing_path = paths.clone();
    missing_path.paths.pop();
    rejects(&missing_path);
    let mut extra_path = paths.clone();
    extra_path.paths.push(paths.paths[0].clone());
    rejects(&extra_path);
    let mut reordered_paths = paths.clone();
    reordered_paths.paths.swap(0, 1);
    rejects(&reordered_paths);
    let mut duplicate_path = paths.clone();
    duplicate_path.paths[1] = paths.paths[0].clone();
    rejects(&duplicate_path);
    let mut wrong_path_identity = paths.clone();
    wrong_path_identity.paths[0].outer_member_identity = paths.paths[1].outer_member_identity;
    rejects(&wrong_path_identity);
    let mut missing_child = paths.clone();
    missing_child.paths[0].child_sum_layouts.pop();
    rejects(&missing_child);
    let mut extra_child = paths.clone();
    extra_child.paths[0]
        .child_sum_layouts
        .push(paths.paths[0].child_sum_layouts[0].clone());
    rejects(&extra_child);
    let mut reordered_children = paths.clone();
    reordered_children.paths[0].child_sum_layouts.swap(0, 1);
    rejects(&reordered_children);
    let mut duplicate_child = paths.clone();
    duplicate_child.paths[0].child_sum_layouts[1] = paths.paths[0].child_sum_layouts[0].clone();
    rejects(&duplicate_child);
    let mut wrong_child_identity = paths.clone();
    wrong_child_identity.paths[0].child_sum_layouts[0].member_identity =
        paths.paths[0].child_sum_layouts[1].member_identity;
    rejects(&wrong_child_identity);
    let mut wrong_outer_layout = paths.clone();
    wrong_outer_layout.outer_layout.entries[2].placement = LayoutPlacementReport::At { offset: 26 };
    rejects(&wrong_outer_layout);
    let mut wrong_inner_layout = paths.clone();
    wrong_inner_layout.paths[1].inner_layout.entries[1].placement =
        LayoutPlacementReport::At { offset: 10 };
    rejects(&wrong_inner_layout);
    let mut wrong_child_geometry = paths.clone();
    wrong_child_geometry.paths[1].child_sum_layouts[0]
        .layout
        .cases[1]
        .payload_fields[0]
        .offset += 1;
    rejects(&wrong_child_geometry);
    assert!(
        carrier
            .replay_against(&checked, "Outer", &paths, &value, ByteOrder::BigEndian,)
            .is_err()
    );
    let mut wrong_value = value.clone();
    let BuildTimeValue::Struct { fields, .. } = &mut wrong_value else {
        unreachable!("fixture is outer record")
    };
    let BuildTimeValue::Struct { fields, .. } = &mut fields[3].1 else {
        unreachable!("second occurrence is inner record")
    };
    fields[0].1 = empty();
    assert!(
        carrier
            .replay_against(
                &checked,
                "Outer",
                &paths,
                &wrong_value,
                ByteOrder::LittleEndian,
            )
            .is_err()
    );
}

#[test]
fn producer_reachability_validates_siblings_after_an_already_found_sum() {
    let checked = checked(
        r#"
        data Choice [copy] { case Empty; }
        data Trap [copy] { choice: Choice; later: u8; }
        data Root [copy] { trap: Trap; }
        "#,
    );
    let trap = checked
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Trap")
        .expect("Trap definition");
    let trap_symbol = trap.symbol;
    let trap_name = trap.name.clone();
    let later_type = checked
        .data_members(trap)
        .iter()
        .find_map(|member| match member {
            DataMember::Field(field) if field.name.as_str() == "later" => {
                Some(field.type_reference)
            }
            DataMember::Field(_) | DataMember::Variant(_) => None,
        })
        .expect("later field");
    let root = checked
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Root")
        .expect("Root definition");
    let trap_type = checked
        .data_members(root)
        .iter()
        .find_map(|member| match member {
            DataMember::Field(field) if field.name.as_str() == "trap" => Some(field.type_reference),
            DataMember::Field(_) | DataMember::Variant(_) => None,
        })
        .expect("trap field");
    assert!(matches!(
        SumReachability::new(&checked).type_contains_sum(trap_type),
        Ok(true)
    ));

    let mut recursive = checked.clone();
    recursive.typed.type_reference_table.substitute_node(
        later_type,
        TypeReferenceNode::Named {
            symbol: trap_symbol,
            name: trap_name.clone(),
        },
    );
    assert!(
        SumReachability::new(&recursive)
            .type_contains_sum(trap_type)
            .is_err(),
        "a later recursive sibling must not hide behind an earlier sum"
    );

    let mut malformed = checked;
    malformed.typed.type_reference_table.substitute_node(
        later_type,
        TypeReferenceNode::Named {
            symbol: SymbolHandle::invalid(),
            name: trap_name,
        },
    );
    assert!(
        SumReachability::new(&malformed)
            .type_contains_sum(trap_type)
            .is_err(),
        "a later malformed nominal branch must not hide behind an earlier sum"
    );
}

#[test]
fn nested_record_path_keeps_erased_values_semantic_but_nonphysical() {
    let checked = checked(
        r#"
        data Choice [copy] { case Empty; case Number(value: u8); }
        data Inner [copy] { choice: Choice; proof [erased]: u64; }
        data Outer [copy] {
            prefix: u8;
            inner: Inner;
            witness [erased]: u32;
            suffix: u16;
        }
        "#,
    );
    let outer = checked
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Outer")
        .unwrap();
    let plan = crate::build_layout_plan(&checked, NativeTarget::host(), &[]).unwrap();
    let path = project_conventional_record_with_nested_sum_record_materialization_layout(
        &checked,
        &plan,
        outer.symbol,
    )
    .expect("erased fields do not create physical path rows");
    assert_eq!(path.outer_layout.entries.len(), 3);
    assert_eq!(path.inner_layout.entries.len(), 1);
    assert_eq!(path.child_sum_layouts.len(), 1);

    let value = BuildTimeValue::Struct {
        type_name: "Outer".into(),
        fields: vec![
            ("prefix".into(), BuildTimeValue::Int(7)),
            (
                "inner".into(),
                BuildTimeValue::Struct {
                    type_name: "Inner".into(),
                    fields: vec![
                        (
                            "choice".into(),
                            BuildTimeValue::Case {
                                variant: "Number".into(),
                                payload: vec![("value".into(), BuildTimeValue::Int(0x5c))],
                            },
                        ),
                        ("proof".into(), BuildTimeValue::Int(99)),
                    ],
                },
            ),
            ("witness".into(), BuildTimeValue::Int(17)),
            ("suffix".into(), BuildTimeValue::Int(0x1122)),
        ],
    };
    let carrier = validate_const_materializable_record_with_nested_sum_record(
        &checked,
        "Outer",
        &path,
        &value,
        ByteOrder::LittleEndian,
    )
    .expect("valid erased values remain required without occupying bytes");
    assert_eq!(
        carrier.bytes(),
        &[7, 0, 0, 0, 1, 0, 0, 0, 0x5c, 0, 0, 0, 0x22, 0x11, 0, 0]
    );

    let mut malformed_erased = value.clone();
    let BuildTimeValue::Struct { fields, .. } = &mut malformed_erased else {
        unreachable!("fixture is outer record")
    };
    fields[2].1 = BuildTimeValue::Bool(true);
    assert!(
        validate_const_materializable_record_with_nested_sum_record(
            &checked,
            "Outer",
            &path,
            &malformed_erased,
            ByteOrder::LittleEndian,
        )
        .is_err(),
        "erased fields remain part of exact typed value validation"
    );
}

#[test]
fn target_layout_projects_one_live_record_with_sum_materialization_pair() {
    let checked = checked(
        r#"
        data Choice [copy] {
            case Empty;
            case Pair(left: u16, right: u32);
        }
        data Envelope [copy] {
            prefix: u8;
            choice: Choice;
            suffix: u16;
        }
        "#,
    );
    let definition = checked
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Envelope")
        .unwrap();
    let plan = crate::build_layout_plan(&checked, NativeTarget::host(), &[]).unwrap();
    let (outer, nested_rows) = project_conventional_record_with_sum_materialization_layout(
        &checked,
        &plan,
        definition.symbol,
    )
    .expect("target runtime layout should project the exact paired evidence");
    assert_eq!(outer.offsets.as_deref(), Some(&[0, 4, 16][..]));
    assert_eq!(outer.size, Some(20));
    assert_eq!(outer.align, 4);
    assert_eq!(nested_rows.len(), 1);
    assert_eq!(nested_rows[0].field, "choice");
    assert_eq!(nested_rows[0].layout.size, 12);
    assert_eq!(nested_rows[0].layout.align, 4);

    let value = BuildTimeValue::Struct {
        type_name: "Envelope".into(),
        fields: vec![
            ("prefix".into(), BuildTimeValue::Int(7)),
            (
                "choice".into(),
                BuildTimeValue::Case {
                    variant: "Pair".into(),
                    payload: vec![
                        ("left".into(), BuildTimeValue::Int(0x1122)),
                        ("right".into(), BuildTimeValue::Int(0x3344_5566)),
                    ],
                },
            ),
            ("suffix".into(), BuildTimeValue::Int(0x7788)),
        ],
    };
    let materialized = validate_const_materializable_record_with_conventional_sum(
        &checked,
        "Envelope",
        &outer,
        &nested_rows[0].layout,
        &value,
        ByteOrder::LittleEndian,
    )
    .expect("the target-produced pair should feed the nested-sum carrier");
    assert_eq!(
        materialized.bytes(),
        &[
            7, 0, 0, 0, 1, 0, 0, 0, 0x22, 0x11, 0, 0, 0x66, 0x55, 0x44, 0x33, 0x88, 0x77, 0, 0,
        ]
    );
}

#[test]
fn target_layout_projects_every_direct_sum_occurrence_and_keeps_broader_shapes_fenced() {
    let checked = checked(
        r#"
        data Choice [copy] { case Empty; case Number(value: u8); }
        data Multiple [copy] { first: Choice; second: Choice; }
        data ErasedAlso [copy] { live: Choice; proof [erased]: Choice; }
        data ArrayOwner [copy] { choices: [Choice; 2]; }
        data ArrayWithNeighbor [copy] { bytes: [u8; 2]; choices: [Choice; 2]; suffix: u16; }
        data ZeroArrayOwner [copy] { choices: [Choice; 0]; }
        data TwoArrayOwner [copy] { first: [Choice; 1]; second: [Choice; 2]; }
        data DirectAndNestedArrayOwner [copy] {
            direct: [Choice; 1];
            nested: [[Choice; 1]; 1];
        }
        data Inner [copy] { choice: Choice; }
        data RecursiveOwner [copy] { inner: Inner; }
        data Mixed [copy] { common: u8; case Empty; }
        data MixedOwner [copy] { mixed: Mixed; }
        data MixedArray [copy] { mixed: [Mixed; 2]; }
        "#,
    );
    let plan = crate::build_layout_plan(&checked, NativeTarget::host(), &[]).unwrap();
    let multiple = checked
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Multiple")
        .unwrap();
    let (outer, nested_rows) = project_conventional_record_with_sum_materialization_layout(
        &checked,
        &plan,
        multiple.symbol,
    )
    .expect("all direct runtime sum occurrences should project in authored order");
    assert_eq!(outer.offsets.as_deref(), Some(&[0, 8][..]));
    assert_eq!(
        nested_rows
            .iter()
            .map(|row| row.field.as_str())
            .collect::<Vec<_>>(),
        ["first", "second"]
    );
    assert_eq!(nested_rows[0].member_identity, None);
    assert_eq!(nested_rows[1].member_identity, None);
    assert_eq!(nested_rows[0].layout, nested_rows[1].layout);

    let erased_also = checked
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "ErasedAlso")
        .unwrap();
    let (erased_outer, erased_rows) = project_conventional_record_with_sum_materialization_layout(
        &checked,
        &plan,
        erased_also.symbol,
    )
    .expect("erased sum fields are not runtime materialization occurrences");
    assert_eq!(erased_outer.offsets.as_deref(), Some(&[0][..]));
    assert_eq!(
        erased_rows
            .iter()
            .map(|row| row.field.as_str())
            .collect::<Vec<_>>(),
        ["live"]
    );

    let array_owner = checked
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "ArrayOwner")
        .unwrap();
    let (array_outer, array_row) =
        project_conventional_record_with_sum_array_materialization_layout(
            &checked,
            &plan,
            array_owner.symbol,
        )
        .expect("one direct nonzero literal sum array should project compactly");
    assert_eq!(array_outer.offsets.as_deref(), Some(&[0][..]));
    assert_eq!(array_row.field, "choices");
    assert_eq!(array_row.member_identity, None);
    assert_eq!(array_row.element_count, 2);
    assert_eq!(array_row.element_stride, array_row.element_layout.size);
    assert_eq!(array_row.element_stride, 8);

    let two_array_owner = checked
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "TwoArrayOwner")
        .unwrap();
    let (two_array_outer, two_array_rows) =
        project_conventional_record_with_sum_arrays_materialization_layout(
            &checked,
            &plan,
            two_array_owner.symbol,
        )
        .expect("every direct sum-array occurrence should project in authored order");
    assert_eq!(two_array_outer.offsets.as_deref(), Some(&[0, 8][..]));
    assert_eq!(two_array_outer.size, Some(24));
    assert_eq!(
        two_array_rows
            .iter()
            .map(|row| (row.field.as_str(), row.element_count, row.element_stride))
            .collect::<Vec<_>>(),
        [("first", 1, 8), ("second", 2, 8)]
    );
    let two_array_value = BuildTimeValue::Struct {
        type_name: "TwoArrayOwner".into(),
        fields: vec![
            (
                "first".into(),
                BuildTimeValue::Array(vec![BuildTimeValue::Case {
                    variant: "Number".into(),
                    payload: vec![("value".into(), BuildTimeValue::Int(0x11))],
                }]),
            ),
            (
                "second".into(),
                BuildTimeValue::Array(vec![
                    BuildTimeValue::Case {
                        variant: "Empty".into(),
                        payload: Vec::new(),
                    },
                    BuildTimeValue::Case {
                        variant: "Number".into(),
                        payload: vec![("value".into(), BuildTimeValue::Int(0x22))],
                    },
                ]),
            ),
        ],
    };
    let two_array_materialized = validate_const_materializable_record_with_conventional_sum_arrays(
        &checked,
        "TwoArrayOwner",
        &two_array_outer,
        &two_array_rows,
        &two_array_value,
        ByteOrder::LittleEndian,
    )
    .expect("the plural target report should rejoin plural value custody");
    assert_eq!(
        two_array_materialized.bytes(),
        &[
            1, 0, 0, 0, 0x11, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0x22, 0, 0, 0,
        ]
    );
    let two_array_layout =
        unique_data_layout(&plan, two_array_owner.symbol, "TwoArrayOwner").unwrap();
    let DataShape::Record {
        fields: two_array_fields,
    } = two_array_layout.shape
    else {
        unreachable!("fixture is a record")
    };
    let second_array_symbol = plan.fields.span_or_empty(two_array_fields)[1].symbol;
    let mut repeated_second_array_plan = plan.clone();
    repeated_second_array_plan
        .repeated_fields
        .push(crate::RepeatedFieldLayout {
            field: second_array_symbol,
            element_stride: 16,
        });
    assert!(
        project_conventional_record_with_sum_arrays_materialization_layout(
            &checked,
            &repeated_second_array_plan,
            two_array_owner.symbol,
        )
        .is_err(),
        "a repeated placement escaping the record's own extent must reject"
    );
    // At the packed stride the repeated vocabulary projects exactly: `second`
    // retains one `At` entry per element and the row keeps the physical
    // stride, while `offsets` retires because not every field has one whole
    // `At` extent.
    let mut packed_second_array_plan = plan.clone();
    packed_second_array_plan
        .repeated_fields
        .push(crate::RepeatedFieldLayout {
            field: second_array_symbol,
            element_stride: 8,
        });
    let (packed_outer, packed_rows) =
        project_conventional_record_with_sum_arrays_materialization_layout(
            &checked,
            &packed_second_array_plan,
            two_array_owner.symbol,
        )
        .expect("repeated placement at the packed stride projects per-element entries");
    assert_eq!(
        packed_outer
            .entries
            .iter()
            .map(|entry| (entry.field.as_str(), entry.placement))
            .collect::<Vec<_>>(),
        [
            ("first", LayoutPlacementReport::At { offset: 0 }),
            ("second", LayoutPlacementReport::At { offset: 8 }),
            ("second", LayoutPlacementReport::At { offset: 16 }),
        ]
    );
    assert_eq!(packed_outer.offsets, None);
    assert_eq!(packed_rows.len(), 2);
    assert_eq!(packed_rows[1].element_count, 2);
    assert_eq!(packed_rows[1].element_stride, 8);

    let array_data_layout = unique_data_layout(&plan, array_owner.symbol, "ArrayOwner").unwrap();
    let DataShape::Record {
        fields: array_fields,
    } = array_data_layout.shape
    else {
        unreachable!("fixture is a record")
    };
    let mut substituted_plan = plan.clone();
    substituted_plan.fields.span_mut_or_empty(array_fields)[0].type_symbol =
        SymbolHandle::invalid();
    assert!(
        project_conventional_record_with_sum_array_materialization_layout(
            &checked,
            &substituted_plan,
            array_owner.symbol,
        )
        .is_err(),
        "an inconsistent laid array element symbol must reject"
    );

    let neighbor_owner = checked
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "ArrayWithNeighbor")
        .unwrap();
    let (neighbor_outer, neighbor_row) =
        project_conventional_record_with_sum_array_materialization_layout(
            &checked,
            &plan,
            neighbor_owner.symbol,
        )
        .expect("the compact target report should preserve ordinary sibling fields");
    let materialized = validate_const_materializable_record_with_conventional_sum_array(
        &checked,
        "ArrayWithNeighbor",
        &neighbor_outer,
        &neighbor_row,
        &BuildTimeValue::Struct {
            type_name: "ArrayWithNeighbor".into(),
            fields: vec![
                (
                    "bytes".into(),
                    BuildTimeValue::Array(vec![
                        BuildTimeValue::Int(0xaa),
                        BuildTimeValue::Int(0xbb),
                    ]),
                ),
                (
                    "choices".into(),
                    BuildTimeValue::Array(vec![
                        BuildTimeValue::Case {
                            variant: "Empty".into(),
                            payload: Vec::new(),
                        },
                        BuildTimeValue::Case {
                            variant: "Number".into(),
                            payload: vec![("value".into(), BuildTimeValue::Int(0x5c))],
                        },
                    ]),
                ),
                ("suffix".into(), BuildTimeValue::Int(0x1122)),
            ],
        },
        ByteOrder::LittleEndian,
    )
    .expect("the target-produced compact report should rejoin indexed materialization");
    assert_eq!(neighbor_outer.offsets.as_deref(), Some(&[0, 4, 20][..]));
    assert_eq!(
        materialized.bytes(),
        &[
            0xaa, 0xbb, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0x5c, 0, 0, 0, 0x22, 0x11, 0, 0,
        ]
    );
    let neighbor_data_layout =
        unique_data_layout(&plan, neighbor_owner.symbol, "ArrayWithNeighbor").unwrap();
    let DataShape::Record {
        fields: neighbor_fields,
    } = neighbor_data_layout.shape
    else {
        unreachable!("fixture is a record")
    };
    let neighbor_field_symbol = plan.fields.span_or_empty(neighbor_fields)[0].symbol;
    let mut repeated_neighbor_plan = plan.clone();
    repeated_neighbor_plan
        .repeated_fields
        .push(crate::RepeatedFieldLayout {
            field: neighbor_field_symbol,
            element_stride: 2,
        });
    let (repeated_outer, _) = project_conventional_record_with_sum_array_materialization_layout(
        &checked,
        &repeated_neighbor_plan,
        neighbor_owner.symbol,
    )
    .expect("repeated placement on a neighboring field transcribes per-element entries");
    assert_eq!(
        repeated_outer
            .entries
            .iter()
            .map(|entry| (entry.field.as_str(), entry.placement))
            .collect::<Vec<_>>(),
        [
            ("bytes", LayoutPlacementReport::At { offset: 0 }),
            ("bytes", LayoutPlacementReport::At { offset: 2 }),
            ("choices", LayoutPlacementReport::At { offset: 4 }),
            ("suffix", LayoutPlacementReport::At { offset: 20 }),
        ]
    );
    assert_eq!(
        repeated_outer.offsets, None,
        "a repeated field's per-element placements retire the one-offset-per-field projection"
    );

    // Scalar target-dependent placements transcribe exactly as well: a
    // stored-integer `suffix` keeps its physical width and interpretation,
    // and a fragmented `suffix` keeps every `Bits` row. Either vocabulary
    // retires `offsets`, which exists only when every field carries one
    // whole `At` extent.
    let suffix_field_symbol = plan.fields.span_or_empty(neighbor_fields)[2].symbol;
    let mut integer_neighbor_plan = plan.clone();
    integer_neighbor_plan
        .stored_integers
        .push(crate::StoredIntegerLayout {
            field: suffix_field_symbol,
            stored_width_bits: 24,
            interpretation: layout_plans::IntegerInterpretation::Signed,
            write_is_total: false,
        });
    let (integer_outer, _) = project_conventional_record_with_sum_array_materialization_layout(
        &checked,
        &integer_neighbor_plan,
        neighbor_owner.symbol,
    )
    .expect("stored-integer placement on a scalar neighbor transcribes its exact entry");
    assert_eq!(
        integer_outer
            .entries
            .iter()
            .map(|entry| (entry.field.as_str(), entry.placement))
            .collect::<Vec<_>>(),
        [
            ("bytes", LayoutPlacementReport::At { offset: 0 }),
            ("choices", LayoutPlacementReport::At { offset: 4 }),
            (
                "suffix",
                LayoutPlacementReport::IntegerAt {
                    offset: 20,
                    stored_width: 24,
                    interpretation: layout_plans::IntegerInterpretation::Signed,
                },
            ),
        ]
    );
    assert_eq!(integer_outer.offsets, None);

    let mut bits_neighbor_plan = plan.clone();
    bits_neighbor_plan.bit_fields.push(crate::BitFieldLayout {
        field: suffix_field_symbol,
        fragments: vec![
            crate::BitFieldFragment {
                container_byte_offset: 20,
                container_width_bits: 8,
                destination_lsb: 0,
                source_lsb: 0,
                width: 8,
            },
            crate::BitFieldFragment {
                container_byte_offset: 21,
                container_width_bits: 8,
                destination_lsb: 8,
                source_lsb: 8,
                width: 8,
            },
        ],
    });
    let (bits_outer, _) = project_conventional_record_with_sum_array_materialization_layout(
        &checked,
        &bits_neighbor_plan,
        neighbor_owner.symbol,
    )
    .expect("bit-fragment placement on a scalar neighbor transcribes every fragment");
    assert_eq!(
        bits_outer
            .entries
            .iter()
            .map(|entry| (entry.field.as_str(), entry.placement))
            .collect::<Vec<_>>(),
        [
            ("bytes", LayoutPlacementReport::At { offset: 0 }),
            ("choices", LayoutPlacementReport::At { offset: 4 }),
            (
                "suffix",
                LayoutPlacementReport::Bits {
                    container: 20,
                    container_width: 8,
                    destination_lsb: 0,
                    source_lsb: 0,
                    width: 8,
                },
            ),
            (
                "suffix",
                LayoutPlacementReport::Bits {
                    container: 21,
                    container_width: 8,
                    destination_lsb: 8,
                    source_lsb: 8,
                    width: 8,
                },
            ),
        ]
    );
    assert_eq!(bits_outer.offsets, None);

    let multiple_layout = unique_data_layout(&plan, multiple.symbol, "Multiple").unwrap();
    let DataShape::Record {
        fields: multiple_fields,
    } = multiple_layout.shape
    else {
        unreachable!("fixture is a record")
    };
    let direct_field_symbol = plan.fields.span_or_empty(multiple_fields)[0].symbol;
    let mut repeated_direct_plan = plan.clone();
    repeated_direct_plan
        .repeated_fields
        .push(crate::RepeatedFieldLayout {
            field: direct_field_symbol,
            element_stride: 16,
        });
    assert!(
        project_conventional_record_with_sum_materialization_layout(
            &checked,
            &repeated_direct_plan,
            multiple.symbol,
        )
        .is_err(),
        "repeated placement on a field that is not a literal fixed array must reject"
    );

    for name in [
        "ZeroArrayOwner",
        "TwoArrayOwner",
        "RecursiveOwner",
        "MixedOwner",
    ] {
        let definition = checked
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == name)
            .unwrap();
        assert!(
            project_conventional_record_with_sum_array_materialization_layout(
                &checked,
                &plan,
                definition.symbol,
            )
            .is_err(),
            "{name} must remain outside the single direct nonzero sum-array rung"
        );
    }

    for name in [
        "ZeroArrayOwner",
        "DirectAndNestedArrayOwner",
        "RecursiveOwner",
        "MixedOwner",
    ] {
        let definition = checked
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == name)
            .unwrap();
        assert!(
            project_conventional_record_with_sum_arrays_materialization_layout(
                &checked,
                &plan,
                definition.symbol,
            )
            .is_err(),
            "{name} must remain outside the plural direct sum-array rung"
        );
    }

    for name in ["ArrayOwner", "RecursiveOwner"] {
        let definition = checked
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == name)
            .unwrap();
        assert!(
            project_conventional_record_with_sum_materialization_layout(
                &checked,
                &plan,
                definition.symbol,
            )
            .is_err(),
            "{name} must remain outside the direct nested-sum rung"
        );
    }

    // A direct mixed member is one case-bearing child of the level: the same
    // compact row carries it, and its report spells the common fields.
    let mixed_owner = checked
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "MixedOwner")
        .unwrap();
    let (mixed_outer, mixed_rows) = project_conventional_record_with_sum_materialization_layout(
        &checked,
        &plan,
        mixed_owner.symbol,
    )
    .expect("a direct mixed member projects under the same nested-sum rung");
    assert_eq!(mixed_outer.offsets.as_deref(), Some(&[0][..]));
    assert_eq!(mixed_rows.len(), 1);
    assert_eq!(mixed_rows[0].field, "mixed");
    assert_eq!(mixed_rows[0].layout.common_fields.len(), 1);
    assert_eq!(mixed_rows[0].layout.common_fields[0].field, "common");
    assert_eq!(mixed_rows[0].layout.common_fields[0].offset, 4);
    assert_eq!(mixed_rows[0].layout.cases.len(), 1);
    assert_eq!(mixed_rows[0].layout.size, 8);
}

#[test]
fn mixed_sum_array_elements_project_with_common_fields_beside_the_overlay() {
    let checked = checked(
        r#"
        data Event [copy] {
            sequence: u8;
            case Ready(value: u16);
            case Waiting;
        }
        data Log [copy] { events: [Event; 2]; }
        "#,
    );
    let plan = crate::build_layout_plan(&checked, NativeTarget::host(), &[]).unwrap();
    let owner = checked
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Log")
        .unwrap();
    let (outer, row) = project_conventional_record_with_sum_array_materialization_layout(
        &checked,
        &plan,
        owner.symbol,
    )
    .expect("a literal array of mixed elements projects under the same compact row");
    assert_eq!(outer.offsets.as_deref(), Some(&[0][..]));
    assert_eq!(outer.size, Some(16));
    assert_eq!(row.field, "events");
    assert_eq!(row.element_count, 2);
    assert_eq!(row.element_stride, 8);
    // The element report spells the mixed interior once: the common field
    // packs between the tag and the shared payload base inside every element.
    assert_eq!(row.element_layout.size, 8);
    assert_eq!(
        row.element_layout
            .common_fields
            .iter()
            .map(|field| (field.field.as_str(), field.offset, field.size))
            .collect::<Vec<_>>(),
        [("sequence", 4, 1)]
    );
    assert_eq!(row.element_layout.cases[0].payload_fields[0].offset, 6);

    // The compact report materializes each element's common field and case
    // payload at its exact offsets inside the packed stride.
    let value = BuildTimeValue::Struct {
        type_name: "Log".into(),
        fields: vec![(
            "events".into(),
            BuildTimeValue::Array(vec![
                BuildTimeValue::Case {
                    variant: "Ready".into(),
                    payload: vec![
                        ("sequence".into(), BuildTimeValue::Int(9)),
                        ("value".into(), BuildTimeValue::Int(0x1122)),
                    ],
                },
                BuildTimeValue::Case {
                    variant: "Waiting".into(),
                    payload: vec![("sequence".into(), BuildTimeValue::Int(7))],
                },
            ]),
        )],
    };
    let materialized = validate_const_materializable_record_with_conventional_sum_array(
        &checked,
        "Log",
        &outer,
        &row,
        &value,
        ByteOrder::LittleEndian,
    )
    .expect("authoritative mixed element report should materialize each element");
    assert_eq!(
        materialized.bytes(),
        &[
            // element 0: tag 0 (Ready), sequence 9, pad, value 0x1122 LE
            0, 0, 0, 0, 9, 0, 0x22, 0x11, // element 1: tag 1 (Waiting), sequence 7
            1, 0, 0, 0, 7, 0, 0, 0,
        ]
    );
}
