//! Const sum materializable layout plan tests.

use super::{
    BuildTimeValue, ByteOrder, ConventionalSumCaseLayoutReport, ConventionalSumLayoutReport,
    DataMember, TypedTrees, checked_align_up,
    normalized_conventional_sum_layout_report_fingerprint, normalized_schema_report_fingerprint,
    reflected_nested_member_layout, unique_data_by_name,
    validate_const_materializable_conventional_sum,
};
use layout_plans::ConventionalSumPayloadFieldLayoutReport;
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
use tokens_to_syntax_trees::parse_syntax_trees;

const SOURCE: &str = r#"
        data Inner [copy] { enabled: bool; code: u32; }
        data Choice [copy] {
            case Empty;
            case Number(value: u8);
            case Nested(inner: Inner);
            case Wide(code: u32, flag: u8);
        }
        data FloatingChoice [copy] { case Empty; case Floating(value: f64); }
        data BorrowedChoice [copy] { case Empty; case Borrowed(value: &u8); }
        data MixedChoice [copy] { common: u8; case Empty; case Number(value: u8); }
    "#;

fn typed() -> TypedTrees {
    let tokens = Lexer::new(SOURCE).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    lower_symbol_resolved_trees(&resolved).expect("type")
}

fn layout(typed: &TypedTrees, schema: &str) -> ConventionalSumLayoutReport {
    let data = unique_data_by_name(typed, schema).unwrap();
    let members = typed.data_members(data);
    let cases = members
        .iter()
        .filter_map(|member| match member {
            DataMember::Variant(variant) => Some(variant),
            DataMember::Field(_) => None,
        })
        .collect::<Vec<_>>();
    let mut max_align = 1;
    let mut shapes = Vec::new();
    for case in &cases {
        let fields = typed
            .data_payload_fields(case)
            .iter()
            .filter(|field| !field.relevance.is_erased())
            .map(|field| {
                let (size, align) = reflected_nested_member_layout(
                    typed,
                    field.type_reference,
                    &mut vec![data.symbol],
                )
                .unwrap();
                max_align = max_align.max(align);
                (field, size, align)
            })
            .collect::<Vec<_>>();
        shapes.push(fields);
    }
    // Common fields pack right after the tag; their aligned end is the floor
    // under the shared payload overlay.
    let mut common_align = 1;
    let mut common_end = 4;
    let common_fields = members
        .iter()
        .filter_map(|member| match member {
            DataMember::Field(field) if !field.relevance.is_erased() => Some(field),
            DataMember::Field(_) | DataMember::Variant(_) => None,
        })
        .map(|field| {
            let (size, align) =
                reflected_nested_member_layout(typed, field.type_reference, &mut vec![data.symbol])
                    .unwrap();
            common_align = common_align.max(align);
            common_end = checked_align_up(common_end, align).unwrap();
            let report = ConventionalSumPayloadFieldLayoutReport {
                field: field.name.to_string(),
                member_identity: field.identity,
                offset: common_end,
                size,
                align,
            };
            common_end += size;
            report
        })
        .collect::<Vec<_>>();
    let common_end = checked_align_up(common_end, common_align).unwrap().max(4);
    let payload_base = checked_align_up(common_end, max_align).unwrap();
    let mut max_end = common_end;
    let reports = cases
        .iter()
        .zip(shapes)
        .enumerate()
        .map(|(ordinal, (case, fields))| {
            let mut offset = payload_base;
            let payload_fields = fields
                .into_iter()
                .map(|(field, size, align)| {
                    offset = checked_align_up(offset, align).unwrap();
                    let report = ConventionalSumPayloadFieldLayoutReport {
                        field: field.name.to_string(),
                        member_identity: field.identity,
                        offset,
                        size,
                        align,
                    };
                    offset += size;
                    report
                })
                .collect();
            max_end = max_end.max(offset);
            ConventionalSumCaseLayoutReport {
                case: case.name.to_string(),
                member_identity: case.identity,
                ordinal: ordinal as u32,
                payload_fields,
            }
        })
        .collect();
    let align = 4.max(common_align).max(max_align);
    ConventionalSumLayoutReport {
        schema_report_fingerprint: normalized_schema_report_fingerprint(typed, data),
        tag_offset: 0,
        tag_size: 4,
        tag_align: 4,
        common_fields,
        cases: reports,
        size: checked_align_up(max_end, align).unwrap(),
        align,
    }
}

#[test]
fn active_case_writes_tag_payload_and_zero_padding_in_both_byte_orders() {
    let typed = typed();
    let layout = layout(&typed, "Choice");
    let value = BuildTimeValue::Case {
        variant: "Wide".into(),
        payload: vec![
            ("code".into(), BuildTimeValue::Int(0x1122_3344)),
            ("flag".into(), BuildTimeValue::Int(7)),
        ],
    };
    let little = validate_const_materializable_conventional_sum(
        &typed,
        "Choice",
        &layout,
        &value,
        ByteOrder::LittleEndian,
    )
    .unwrap();
    assert_eq!(little.selected_case_ordinal(), 3);
    assert_ne!(little.non_authoritative_schema_report_fingerprint(), 0);
    assert_ne!(little.non_authoritative_layout_report_fingerprint(), 0);
    assert_ne!(
        little.non_authoritative_materialization_report_fingerprint(),
        0
    );
    assert_eq!(
        little.bytes(),
        &[3, 0, 0, 0, 0x44, 0x33, 0x22, 0x11, 7, 0, 0, 0]
    );

    let big = validate_const_materializable_conventional_sum(
        &typed,
        "Choice",
        &layout,
        &value,
        ByteOrder::BigEndian,
    )
    .unwrap();
    assert_eq!(
        big.bytes(),
        &[0, 0, 0, 3, 0x11, 0x22, 0x33, 0x44, 7, 0, 0, 0]
    );
    assert_ne!(
        little.non_authoritative_materialization_report_fingerprint(),
        big.non_authoritative_materialization_report_fingerprint()
    );
}

#[test]
fn payloadless_case_zeros_the_complete_inactive_overlay() {
    let typed = typed();
    let layout = layout(&typed, "Choice");
    let value = BuildTimeValue::Case {
        variant: "Empty".into(),
        payload: Vec::new(),
    };
    let carrier = validate_const_materializable_conventional_sum(
        &typed,
        "Choice",
        &layout,
        &value,
        ByteOrder::LittleEndian,
    )
    .unwrap();
    assert_eq!(carrier.bytes(), &[0; 12]);
}

#[test]
fn active_nested_record_is_encoded_at_the_selected_payload_offset() {
    let typed = typed();
    let layout = layout(&typed, "Choice");
    let value = BuildTimeValue::Case {
        variant: "Nested".into(),
        payload: vec![(
            "inner".into(),
            BuildTimeValue::Struct {
                type_name: "Inner".into(),
                fields: vec![
                    ("enabled".into(), BuildTimeValue::Bool(true)),
                    ("code".into(), BuildTimeValue::Int(0x1020_3040)),
                ],
            },
        )],
    };
    let carrier = validate_const_materializable_conventional_sum(
        &typed,
        "Choice",
        &layout,
        &value,
        ByteOrder::LittleEndian,
    )
    .unwrap();
    assert_eq!(
        carrier.bytes(),
        &[2, 0, 0, 0, 1, 0, 0, 0, 0x40, 0x30, 0x20, 0x10]
    );
}

#[test]
fn replay_rejects_case_layout_value_byte_order_and_byte_drift_atomically() {
    let typed = typed();
    let layout = layout(&typed, "Choice");
    let value = BuildTimeValue::Case {
        variant: "Number".into(),
        payload: vec![("value".into(), BuildTimeValue::Int(9))],
    };
    let mut carrier = validate_const_materializable_conventional_sum(
        &typed,
        "Choice",
        &layout,
        &value,
        ByteOrder::LittleEndian,
    )
    .unwrap();

    let mut drifted = layout.clone();
    drifted.cases[3].payload_fields[0].offset += 1;
    assert!(
        carrier
            .replay_against(&typed, "Choice", &drifted, &value, ByteOrder::LittleEndian)
            .is_err()
    );
    let wrong_value = BuildTimeValue::Case {
        variant: "Empty".into(),
        payload: Vec::new(),
    };
    assert!(
        carrier
            .replay_against(
                &typed,
                "Choice",
                &layout,
                &wrong_value,
                ByteOrder::LittleEndian
            )
            .is_err()
    );
    assert!(
        carrier
            .replay_against(&typed, "Choice", &layout, &value, ByteOrder::BigEndian)
            .is_err()
    );

    let mut short = [0xa5; 11];
    assert!(carrier.apply(&typed, &mut short).is_err());
    assert_eq!(short, [0xa5; 11]);

    carrier.bytes[4] ^= 1;
    let mut destination = [0xa5; 12];
    assert!(carrier.apply(&typed, &mut destination).is_err());
    assert_eq!(destination, [0xa5; 12]);
}

#[test]
fn replay_rejects_sum_layout_substitution_when_compact_report_fingerprint_is_forced_equal() {
    let typed = typed();
    let layout = layout(&typed, "Choice");
    let value = BuildTimeValue::Case {
        variant: "Number".into(),
        payload: vec![("value".into(), BuildTimeValue::Int(9))],
    };
    let mut carrier = validate_const_materializable_conventional_sum(
        &typed,
        "Choice",
        &layout,
        &value,
        ByteOrder::LittleEndian,
    )
    .expect("fixture should validate");

    let mut substituted_layout = layout.clone();
    substituted_layout.cases[3].payload_fields[0].offset += 1;
    carrier.non_authoritative_layout_report_fingerprint =
        normalized_conventional_sum_layout_report_fingerprint(&substituted_layout);

    let error = carrier
        .replay_against(
            &typed,
            "Choice",
            &substituted_layout,
            &value,
            ByteOrder::LittleEndian,
        )
        .expect_err("exact retained sum layout rejects a compact-equal substitute");
    assert!(error.0.contains("layout drifted"));
}

#[test]
fn only_selected_value_payload_is_checked_but_all_case_geometry_replays() {
    let typed = typed();
    let floating_layout = layout(&typed, "FloatingChoice");
    let empty = BuildTimeValue::Case {
        variant: "Empty".into(),
        payload: Vec::new(),
    };
    validate_const_materializable_conventional_sum(
        &typed,
        "FloatingChoice",
        &floating_layout,
        &empty,
        ByteOrder::LittleEndian,
    )
    .expect("inactive f64 case has geometry but no active NaN value to reject");

    let borrowed = unique_data_by_name(&typed, "BorrowedChoice").unwrap();
    let forged = ConventionalSumLayoutReport {
        schema_report_fingerprint: normalized_schema_report_fingerprint(&typed, borrowed),
        tag_offset: 0,
        tag_size: 4,
        tag_align: 4,
        common_fields: Vec::new(),
        cases: vec![
            ConventionalSumCaseLayoutReport {
                case: "Empty".into(),
                member_identity: None,
                ordinal: 0,
                payload_fields: Vec::new(),
            },
            ConventionalSumCaseLayoutReport {
                case: "Borrowed".into(),
                member_identity: None,
                ordinal: 1,
                payload_fields: vec![ConventionalSumPayloadFieldLayoutReport {
                    field: "value".into(),
                    member_identity: None,
                    offset: 8,
                    size: 8,
                    align: 8,
                }],
            },
        ],
        size: 16,
        align: 8,
    };
    let error = validate_const_materializable_conventional_sum(
        &typed,
        "BorrowedChoice",
        &forged,
        &empty,
        ByteOrder::LittleEndian,
    )
    .unwrap_err();
    assert!(
        error
            .0
            .contains("target-independent fixed aggregate subset")
    );
}

#[test]
fn malformed_active_case_payload_and_mixed_shape_fail_closed() {
    let typed = typed();
    let layout = layout(&typed, "Choice");
    for value in [
        BuildTimeValue::Case {
            variant: "Missing".into(),
            payload: Vec::new(),
        },
        BuildTimeValue::Case {
            variant: "Number".into(),
            payload: Vec::new(),
        },
        BuildTimeValue::Case {
            variant: "Number".into(),
            payload: vec![
                ("value".into(), BuildTimeValue::Int(1)),
                ("value".into(), BuildTimeValue::Int(2)),
            ],
        },
    ] {
        assert!(
            validate_const_materializable_conventional_sum(
                &typed,
                "Choice",
                &layout,
                &value,
                ByteOrder::LittleEndian,
            )
            .is_err()
        );
    }

    let empty = BuildTimeValue::Case {
        variant: "Empty".into(),
        payload: Vec::new(),
    };
    let mut bad_tag = layout.clone();
    bad_tag.tag_offset = 1;
    assert!(
        validate_const_materializable_conventional_sum(
            &typed,
            "Choice",
            &bad_tag,
            &empty,
            ByteOrder::LittleEndian,
        )
        .is_err()
    );
    let mut bad_ordinal = layout.clone();
    bad_ordinal.cases[1].ordinal = 2;
    assert!(
        validate_const_materializable_conventional_sum(
            &typed,
            "Choice",
            &bad_ordinal,
            &empty,
            ByteOrder::LittleEndian,
        )
        .is_err()
    );
    let mut bad_extent = layout.clone();
    bad_extent.size += 4;
    assert!(
        validate_const_materializable_conventional_sum(
            &typed,
            "Choice",
            &bad_extent,
            &empty,
            ByteOrder::LittleEndian,
        )
        .is_err()
    );

    let mixed = unique_data_by_name(&typed, "MixedChoice").unwrap();
    let empty_layout = ConventionalSumLayoutReport {
        schema_report_fingerprint: normalized_schema_report_fingerprint(&typed, mixed),
        tag_offset: 0,
        tag_size: 4,
        tag_align: 4,
        common_fields: Vec::new(),
        cases: Vec::new(),
        size: 4,
        align: 4,
    };
    // A mixed value carries its common field inside the merged case payload;
    // omitting it fails the expected member count.
    let error = validate_const_materializable_conventional_sum(
        &typed,
        "MixedChoice",
        &empty_layout,
        &BuildTimeValue::Case {
            variant: "Empty".into(),
            payload: Vec::new(),
        },
        ByteOrder::LittleEndian,
    )
    .unwrap_err();
    assert!(error.0.contains("common fields"));
}

#[test]
fn mixed_shape_writes_common_field_and_selected_case_payload() {
    let typed = typed();
    let layout = layout(&typed, "MixedChoice");
    // common: u8 packs at offset 4 right after the tag; the payload base and
    // Number.value land at offset 5 under its 1-byte alignment.
    assert_eq!(layout.common_fields.len(), 1);
    assert_eq!(layout.common_fields[0].offset, 4);
    assert_eq!(layout.size, 8);
    assert_eq!(layout.align, 4);

    // The merged case payload spells the common field beside the selected
    // case's own payload member.
    let value = BuildTimeValue::Case {
        variant: "Number".into(),
        payload: vec![
            ("common".into(), BuildTimeValue::Int(9)),
            ("value".into(), BuildTimeValue::Int(3)),
        ],
    };
    let carrier = validate_const_materializable_conventional_sum(
        &typed,
        "MixedChoice",
        &layout,
        &value,
        ByteOrder::LittleEndian,
    )
    .unwrap();
    assert_eq!(carrier.selected_case_ordinal(), 1);
    assert_eq!(carrier.bytes(), &[1, 0, 0, 0, 9, 3, 0, 0]);

    // A payload-less case still writes its common field; the shared overlay
    // stays zeroed.
    let empty = BuildTimeValue::Case {
        variant: "Empty".into(),
        payload: vec![("common".into(), BuildTimeValue::Int(7))],
    };
    let carrier = validate_const_materializable_conventional_sum(
        &typed,
        "MixedChoice",
        &layout,
        &empty,
        ByteOrder::LittleEndian,
    )
    .unwrap();
    assert_eq!(carrier.selected_case_ordinal(), 0);
    assert_eq!(carrier.bytes(), &[0, 0, 0, 0, 7, 0, 0, 0]);
}

#[test]
fn mixed_shape_rejects_drifted_common_geometry_and_unmerged_spelling() {
    let typed = typed();
    let layout = layout(&typed, "MixedChoice");
    let value = BuildTimeValue::Case {
        variant: "Number".into(),
        payload: vec![
            ("common".into(), BuildTimeValue::Int(9)),
            ("value".into(), BuildTimeValue::Int(3)),
        ],
    };

    let mut drifted = layout.clone();
    drifted.common_fields[0].offset = 5;
    assert!(
        validate_const_materializable_conventional_sum(
            &typed,
            "MixedChoice",
            &drifted,
            &value,
            ByteOrder::LittleEndian,
        )
        .is_err()
    );

    let mut drifted = layout.clone();
    drifted.common_fields[0].field = "other".into();
    assert!(
        validate_const_materializable_conventional_sum(
            &typed,
            "MixedChoice",
            &drifted,
            &value,
            ByteOrder::LittleEndian,
        )
        .is_err()
    );

    // A payload spelling only the case member misses the common field.
    let missing_common = BuildTimeValue::Case {
        variant: "Number".into(),
        payload: vec![("value".into(), BuildTimeValue::Int(3))],
    };
    assert!(
        validate_const_materializable_conventional_sum(
            &typed,
            "MixedChoice",
            &layout,
            &missing_common,
            ByteOrder::LittleEndian,
        )
        .is_err()
    );

    let carrier = validate_const_materializable_conventional_sum(
        &typed,
        "MixedChoice",
        &layout,
        &value,
        ByteOrder::LittleEndian,
    )
    .unwrap();
    let mut drifted = layout.clone();
    drifted.common_fields[0].offset += 1;
    assert!(
        carrier
            .replay_against(
                &typed,
                "MixedChoice",
                &drifted,
                &value,
                ByteOrder::LittleEndian
            )
            .is_err()
    );
}
