use super::super::{normalized_schema_report_fingerprint, unique_data_by_name};
use super::{
    BuildTimeValue, ByteOrder, ConventionalRecursiveRecordSumPathsLayoutReport, TypedTrees,
    ValidatedConstRecordSumChildMaterialization,
    validate_const_materializable_record_with_recursive_nested_sums,
};
use layout_plans::{
    ConventionalRecordSumChildHop, ConventionalRecordSumChildInterior,
    ConventionalRecordSumChildLayoutReport, ConventionalSumCaseLayoutReport,
    ConventionalSumLayoutReport, ConventionalSumPayloadFieldLayoutReport, LayoutFieldEntryReport,
    LayoutPlacementReport, LayoutPlanReport,
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
    // level spells all three child kinds in authored order on one channel.
    // The record-array row retains the element record's leaf report once
    // beside the literal count and stride its hop carries.
    let leaf_report = || ConventionalRecursiveRecordSumPathsLayoutReport {
        outer_layout: record("Leaf", &[("choice", 0)], 8),
        children: vec![ConventionalRecordSumChildLayoutReport {
            field: "choice".into(),
            member_identity: None,
            hop: ConventionalRecordSumChildHop::Field,
            interior: ConventionalRecordSumChildInterior::Sum(sum.clone()),
        }],
    };
    let report = ConventionalRecursiveRecordSumPathsLayoutReport {
        outer_layout: record("Root", &[("inner", 0), ("route", 8), ("neighbors", 16)], 32),
        children: vec![
            ConventionalRecordSumChildLayoutReport {
                field: "inner".into(),
                member_identity: None,
                hop: ConventionalRecordSumChildHop::Field,
                interior: ConventionalRecordSumChildInterior::Record(leaf_report()),
            },
            ConventionalRecordSumChildLayoutReport {
                field: "route".into(),
                member_identity: None,
                hop: ConventionalRecordSumChildHop::Field,
                interior: ConventionalRecordSumChildInterior::Sum(sum.clone()),
            },
            ConventionalRecordSumChildLayoutReport {
                field: "neighbors".into(),
                member_identity: None,
                hop: ConventionalRecordSumChildHop::Index {
                    element_count: 2,
                    element_stride: 8,
                },
                interior: ConventionalRecordSumChildInterior::Record(leaf_report()),
            },
        ],
    };
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
        match mutation {
            0 => custody.bytes[4] ^= 1,
            1 => {
                let ValidatedConstRecordSumChildMaterialization::Record(occurrence) =
                    &mut custody.children[0]
                else {
                    unreachable!()
                };
                occurrence.outer_field = "substitute".into();
            }
            2 => custody.non_authoritative_materialization_report_fingerprint ^= 1,
            3 => custody.path_layout.children[0].field = "substitute".into(),
            // The coexisting direct-sum row drifts in the retained report.
            4 => custody.path_layout.children[1].field = "substitute".into(),
            // The retained direct-sum custody drifts from the replayed row.
            5 => {
                let ValidatedConstRecordSumChildMaterialization::Sum(sum_custody) =
                    &mut custody.children[1]
                else {
                    unreachable!()
                };
                sum_custody.field = "substitute".into();
            }
            // The record-array row's identity drifts in the retained report.
            6 => custody.path_layout.children[2].field = "substitute".into(),
            // The record-array row's literal geometry drifts in the
            // retained report.
            7 => {
                let ConventionalRecordSumChildHop::Index { element_count, .. } =
                    &mut custody.path_layout.children[2].hop
                else {
                    unreachable!()
                };
                *element_count = 3;
            }
            // The record-array row's shared element report drifts in the
            // retained report.
            8 => {
                let ConventionalRecordSumChildInterior::Record(element) =
                    &mut custody.path_layout.children[2].interior
                else {
                    panic!("record-array element is a record interior")
                };
                element.children[0].field = "substitute".into();
            }
            // The retained record-array custody drifts from the replayed
            // row — the field and one indexed element selection alike.
            9 => {
                let ValidatedConstRecordSumChildMaterialization::RecordArray(array_custody) =
                    &mut custody.children[2]
                else {
                    unreachable!()
                };
                array_custody.field = "substitute".into();
                array_custody.elements[1].literal_index = 9;
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
    let report = ConventionalRecursiveRecordSumPathsLayoutReport {
        outer_layout: record("Root", &[("inner", 0)], 16),
        children: vec![ConventionalRecordSumChildLayoutReport {
            field: "inner".into(),
            member_identity: None,
            hop: ConventionalRecordSumChildHop::Field,
            interior: ConventionalRecordSumChildInterior::Record(
                ConventionalRecursiveRecordSumPathsLayoutReport {
                    outer_layout: record("Leaf", &[("hits", 0)], 16),
                    children: vec![ConventionalRecordSumChildLayoutReport {
                        field: "hits".into(),
                        member_identity: None,
                        hop: ConventionalRecordSumChildHop::Index {
                            element_count: 2,
                            element_stride: 8,
                        },
                        interior: ConventionalRecordSumChildInterior::Sum(mixed),
                    }],
                },
            ),
        }],
    };
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
        let outer_layout = report.outer_layout.clone();
        report = ConventionalRecursiveRecordSumPathsLayoutReport {
            outer_layout,
            children: vec![ConventionalRecordSumChildLayoutReport {
                field: "inner".into(),
                member_identity: None,
                hop: ConventionalRecordSumChildHop::Field,
                interior: ConventionalRecordSumChildInterior::Record(report),
            }],
        };
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
