use super::super::{
    ConventionalRecordSumPathsLayoutReport, normalized_schema_report_fingerprint,
    unique_data_by_name,
};
use super::{
    BuildTimeValue, ByteOrder, ConventionalRecursiveRecordSumPathsLayoutReport, TypedTrees,
    ValidatedConstRecordWithRecursiveNestedSumsMaterialization,
    validate_const_materializable_record_with_recursive_nested_sums,
};
use layout_plans::{
    ConventionalRecordArrayFieldLayoutReport, ConventionalRecordSumOccurrenceLayoutReport,
    ConventionalSumArrayFieldLayoutReport, ConventionalSumCaseLayoutReport,
    ConventionalSumFieldLayoutReport, ConventionalSumLayoutReport,
    ConventionalSumPayloadFieldLayoutReport, LayoutFieldEntryReport, LayoutPlacementReport,
    LayoutPlanReport,
};
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
use tokens_to_syntax_trees::parse_syntax_trees;

fn fixture() -> (
    TypedTrees,
    ConventionalRecursiveRecordSumPathsLayoutReport,
    BuildTimeValue,
) {
    let tokens = Lexer::new("data Choice [copy] { case Zero; case One(value: u8); } data Leaf [copy] { choice: Choice; } data Root [copy] { inner: Leaf; route: Choice; neighbors: [Leaf; 2]; }").tokenize().unwrap();
    let syntax = parse_syntax_trees(&tokens).unwrap();
    let resolved = resolve(ResolutionRequest::new(&syntax)).unwrap();
    let typed = lower_symbol_resolved_trees(&resolved).unwrap();
    let fingerprint = |name| {
        normalized_schema_report_fingerprint(&typed, unique_data_by_name(&typed, name).unwrap())
    };
    let record = |name, fields: &[(&str, u64)], size: u64| LayoutPlanReport {
        schema_report_fingerprint: fingerprint(name),
        entries: fields
            .iter()
            .map(|&(field, offset)| LayoutFieldEntryReport {
                field: field.into(),
                member_identity: None,
                placement: LayoutPlacementReport::At { offset },
            })
            .collect(),
        offsets: Some(fields.iter().map(|&(_, offset)| offset).collect()),
        size: Some(size),
        align: 4,
    };
    let sum = ConventionalSumLayoutReport {
        schema_report_fingerprint: fingerprint("Choice"),
        tag_offset: 0,
        tag_size: 4,
        tag_align: 4,
        size: 8,
        align: 4,
        common_fields: Vec::new(),
        cases: vec![
            ConventionalSumCaseLayoutReport {
                case: "Zero".into(),
                member_identity: None,
                ordinal: 0,
                payload_fields: vec![],
            },
            ConventionalSumCaseLayoutReport {
                case: "One".into(),
                member_identity: None,
                ordinal: 1,
                payload_fields: vec![ConventionalSumPayloadFieldLayoutReport {
                    field: "value".into(),
                    member_identity: None,
                    offset: 4,
                    size: 1,
                    align: 1,
                }],
            },
        ],
    };
    // `Root` co-locates the deeper `inner` record path with its own direct
    // sum `route` and its direct record array `neighbors`, so the outer
    // `Branch` carries all three child kinds. The record-array row retains
    // the element record's leaf report once beside the literal count and
    // stride.
    let leaf_report = || ConventionalRecursiveRecordSumPathsLayoutReport::Leaf {
        outer_layout: record("Leaf", &[("choice", 0)], 8),
        child_sum_layouts: vec![ConventionalSumFieldLayoutReport {
            field: "choice".into(),
            member_identity: None,
            layout: sum.clone(),
        }],
        child_sum_array_layouts: Vec::new(),
        child_record_array_layouts: Vec::new(),
    };
    let report = ConventionalRecursiveRecordSumPathsLayoutReport::Branch(
        ConventionalRecordSumPathsLayoutReport {
            outer_layout: record("Root", &[("inner", 0), ("route", 8), ("neighbors", 16)], 32),
            child_sum_layouts: vec![ConventionalSumFieldLayoutReport {
                field: "route".into(),
                member_identity: None,
                layout: sum.clone(),
            }],
            child_sum_array_layouts: Vec::new(),
            child_record_array_layouts: vec![ConventionalRecordArrayFieldLayoutReport {
                field: "neighbors".into(),
                member_identity: None,
                element_count: 2,
                element_stride: 8,
                inner: leaf_report(),
            }],
            paths: vec![ConventionalRecordSumOccurrenceLayoutReport {
                outer_field: "inner".into(),
                outer_member_identity: None,
                inner: leaf_report(),
            }],
        },
    );
    let leaf_value = |payload: i64| BuildTimeValue::Struct {
        type_name: "Leaf".into(),
        fields: vec![(
            "choice".into(),
            BuildTimeValue::Case {
                variant: "One".into(),
                payload: vec![("value".into(), BuildTimeValue::Int(payload))],
            },
        )],
    };
    let value = BuildTimeValue::Struct {
        type_name: "Root".into(),
        fields: vec![
            ("inner".into(), leaf_value(7)),
            (
                "route".into(),
                BuildTimeValue::Case {
                    variant: "One".into(),
                    payload: vec![("value".into(), BuildTimeValue::Int(9))],
                },
            ),
            (
                "neighbors".into(),
                BuildTimeValue::Array(vec![leaf_value(11), leaf_value(13)]),
            ),
        ],
    };
    (typed, report, value)
}

#[test]
fn retained_recursive_bytes_identity_and_coordinates_are_not_authority() {
    for mutation in 0..10 {
        let (typed, report, value) = fixture();
        let mut custody = validate_const_materializable_record_with_recursive_nested_sums(
            &typed,
            "Root",
            &report,
            &value,
            ByteOrder::LittleEndian,
        )
        .unwrap();
        assert_eq!(
            custody.bytes(),
            &[
                1, 0, 0, 0, 7, 0, 0, 0, // inner.choice = One(7)
                1, 0, 0, 0, 9, 0, 0, 0, // route = One(9)
                1, 0, 0, 0, 11, 0, 0, 0, // neighbors[0].choice = One(11)
                1, 0, 0, 0, 13, 0, 0, 0, // neighbors[1].choice = One(13)
            ]
        );
        let ValidatedConstRecordWithRecursiveNestedSumsMaterialization::Branch(branch) =
            &mut custody
        else {
            panic!("record edge")
        };
        match mutation {
            0 => branch.bytes[4] ^= 1,
            1 => branch.occurrences[0].outer_field = "substitute".into(),
            2 => branch.non_authoritative_materialization_report_fingerprint ^= 1,
            3 => branch.path_layout.paths[0].outer_field = "substitute".into(),
            // The coexisting direct-sum row drifts in the retained report.
            4 => branch.path_layout.child_sum_layouts[0].field = "substitute".into(),
            // The retained direct-sum custody drifts from the replayed row.
            5 => branch.nested_sums[0].field = "substitute".into(),
            // The record-array row's identity drifts in the retained report.
            6 => branch.path_layout.child_record_array_layouts[0].field = "substitute".into(),
            // The record-array row's literal geometry drifts in the
            // retained report.
            7 => branch.path_layout.child_record_array_layouts[0].element_count = 3,
            // The record-array row's shared element report drifts in the
            // retained report.
            8 => {
                let ConventionalRecursiveRecordSumPathsLayoutReport::Leaf {
                    child_sum_layouts, ..
                } = &mut branch.path_layout.child_record_array_layouts[0].inner
                else {
                    panic!("record-array element is a leaf level")
                };
                child_sum_layouts[0].field = "substitute".into();
            }
            // The retained record-array custody drifts from the replayed
            // row — the field and one indexed element selection alike.
            9 => {
                branch.nested_record_arrays[0].field = "substitute".into();
                branch.nested_record_arrays[0].elements[1].literal_index = 9;
            }
            _ => unreachable!(),
        }
        let mut destination = [0xa5; 34];
        assert!(custody.apply(&typed, &mut destination).is_err());
        assert_eq!(destination, [0xa5; 34]);
    }
}

#[test]
fn recursive_mixed_sum_array_materializes_common_and_case_members_per_element() {
    let tokens = Lexer::new("data Mixed [copy] { sequence: u8; case Empty; case Hit(value: u8); } data Leaf [copy] { hits: [Mixed; 2]; } data Root [copy] { inner: Leaf; }").tokenize().unwrap();
    let syntax = parse_syntax_trees(&tokens).unwrap();
    let resolved = resolve(ResolutionRequest::new(&syntax)).unwrap();
    let typed = lower_symbol_resolved_trees(&resolved).unwrap();
    let fingerprint = |name| {
        normalized_schema_report_fingerprint(&typed, unique_data_by_name(&typed, name).unwrap())
    };
    let record = |name, fields: &[(&str, u64)], size: u64| LayoutPlanReport {
        schema_report_fingerprint: fingerprint(name),
        entries: fields
            .iter()
            .map(|&(field, offset)| LayoutFieldEntryReport {
                field: field.into(),
                member_identity: None,
                placement: LayoutPlacementReport::At { offset },
            })
            .collect(),
        offsets: Some(fields.iter().map(|&(_, offset)| offset).collect()),
        size: Some(size),
        align: 4,
    };
    // `Mixed` packs `sequence` beside the tag and overlays `Hit.value` right
    // behind the common extent: 8 bytes at 4-byte alignment per element.
    let mixed = ConventionalSumLayoutReport {
        schema_report_fingerprint: fingerprint("Mixed"),
        tag_offset: 0,
        tag_size: 4,
        tag_align: 4,
        common_fields: vec![ConventionalSumPayloadFieldLayoutReport {
            field: "sequence".into(),
            member_identity: None,
            offset: 4,
            size: 1,
            align: 1,
        }],
        cases: vec![
            ConventionalSumCaseLayoutReport {
                case: "Empty".into(),
                member_identity: None,
                ordinal: 0,
                payload_fields: vec![],
            },
            ConventionalSumCaseLayoutReport {
                case: "Hit".into(),
                member_identity: None,
                ordinal: 1,
                payload_fields: vec![ConventionalSumPayloadFieldLayoutReport {
                    field: "value".into(),
                    member_identity: None,
                    offset: 5,
                    size: 1,
                    align: 1,
                }],
            },
        ],
        size: 8,
        align: 4,
    };
    let report = ConventionalRecursiveRecordSumPathsLayoutReport::Branch(
        ConventionalRecordSumPathsLayoutReport {
            outer_layout: record("Root", &[("inner", 0)], 16),
            child_sum_layouts: Vec::new(),
            child_sum_array_layouts: Vec::new(),
            child_record_array_layouts: Vec::new(),
            paths: vec![ConventionalRecordSumOccurrenceLayoutReport {
                outer_field: "inner".into(),
                outer_member_identity: None,
                inner: ConventionalRecursiveRecordSumPathsLayoutReport::Leaf {
                    outer_layout: record("Leaf", &[("hits", 0)], 16),
                    child_sum_layouts: Vec::new(),
                    child_sum_array_layouts: vec![ConventionalSumArrayFieldLayoutReport {
                        field: "hits".into(),
                        member_identity: None,
                        element_count: 2,
                        element_stride: 8,
                        element_layout: mixed,
                    }],
                    child_record_array_layouts: Vec::new(),
                },
            }],
        },
    );
    // A mixed case value merges its common field with the selected case
    // payload fields in one spelling.
    let value = BuildTimeValue::Struct {
        type_name: "Root".into(),
        fields: vec![(
            "inner".into(),
            BuildTimeValue::Struct {
                type_name: "Leaf".into(),
                fields: vec![(
                    "hits".into(),
                    BuildTimeValue::Array(vec![
                        BuildTimeValue::Case {
                            variant: "Hit".into(),
                            payload: vec![
                                ("sequence".into(), BuildTimeValue::Int(9)),
                                ("value".into(), BuildTimeValue::Int(3)),
                            ],
                        },
                        BuildTimeValue::Case {
                            variant: "Empty".into(),
                            payload: vec![("sequence".into(), BuildTimeValue::Int(7))],
                        },
                    ]),
                )],
            },
        )],
    };
    let custody = validate_const_materializable_record_with_recursive_nested_sums(
        &typed,
        "Root",
        &report,
        &value,
        ByteOrder::LittleEndian,
    )
    .expect("a recursive leaf retains mixed sum-array elements");
    assert_eq!(
        custody.bytes(),
        &[
            1, 0, 0, 0, 9, 3, 0, 0, // hits[0] = Hit(sequence 9, value 3)
            0, 0, 0, 0, 7, 0, 0, 0, // hits[1] = Empty(sequence 7)
        ]
    );
    let mut destination = [0xa5; 16];
    custody
        .apply(&typed, &mut destination)
        .expect("the recursive writer replays the mixed element bytes");
    assert_eq!(destination.as_slice(), custody.bytes());
}

#[test]
fn recursive_resource_bounds_reject_before_typed_derivation() {
    let (typed, mut report, value) = fixture();
    for _ in 0..layout_plans::CONVENTIONAL_RECORD_PATH_DEPTH_LIMIT {
        report = ConventionalRecursiveRecordSumPathsLayoutReport::Branch(
            ConventionalRecordSumPathsLayoutReport {
                outer_layout: report.outer_layout().clone(),
                paths: vec![ConventionalRecordSumOccurrenceLayoutReport {
                    outer_field: "inner".into(),
                    outer_member_identity: None,
                    inner: report,
                }],
                child_sum_layouts: Vec::new(),
                child_sum_array_layouts: Vec::new(),
                child_record_array_layouts: Vec::new(),
            },
        );
    }
    let diagnostic = validate_const_materializable_record_with_recursive_nested_sums(
        &typed,
        "Root",
        &report,
        &value,
        ByteOrder::LittleEndian,
    )
    .unwrap_err();
    assert!(
        diagnostic.0.contains("depth resource bound"),
        "{}",
        diagnostic.0
    );
}
