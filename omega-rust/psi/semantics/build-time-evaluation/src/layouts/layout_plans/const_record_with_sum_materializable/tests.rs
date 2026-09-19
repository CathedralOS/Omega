//! Const record-with-sum materialization tests.

use super::{
    BuildTimeValue, ByteOrder, ConventionalSumArrayFieldLayoutReport,
    ConventionalSumFieldLayoutReport, ConventionalSumLayoutReport, DataMember, LayoutPlanReport,
    TypedTrees, normalized_layout_plan_report_fingerprint, unique_data_by_name,
    validate_const_materializable_record_with_conventional_sum,
    validate_const_materializable_record_with_conventional_sum_array,
    validate_const_materializable_record_with_conventional_sum_arrays,
    validate_const_materializable_record_with_conventional_sums,
};
use layout_plans::{
    ConventionalSumCaseLayoutReport, ConventionalSumPayloadFieldLayoutReport,
    LayoutFieldEntryReport,
};
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
use tokens_to_syntax_trees::parse_syntax_trees;

use crate::layouts::layout_plans::normalized_schema_report_fingerprint;
use crate::layouts::layout_plans::{checked_align_up, reflected_nested_member_layout};
use layout_plans::LayoutPlacementReport;

const SOURCE: &str = r#"
    data Choice [copy] {
        case Empty;
        case Small(value: u16);
        case Wide(code: u32, flag: u8);
    }
    data Envelope [copy] { prefix: u8; choice: Choice; suffix: u16; }
    data TwoChoices [copy] { first: Choice; second: Choice; }
    data ChoiceArray [copy] { choices: [Choice; 2]; }
    data ZeroChoiceArray [copy] { choices: [Choice; 0]; }
    data TwoChoiceArrays [copy] { first: [Choice; 1]; second: [Choice; 2]; }
    data NestedChoiceArray [copy] { choices: [[Choice; 2]; 1]; }
    data DirectAndNestedChoiceArray [copy] {
        direct: [Choice; 1];
        nested: [[Choice; 1]; 1];
    }
    data InnerEnvelope [copy] { choice: Choice; }
    data DeepEnvelope [copy] { inner: InnerEnvelope; }
    data MixedChoice [copy] { common: u8; case Empty; case Number(value: u8); }
    data MixedEnvelope [copy] { choice: MixedChoice; }
    data MixedChoiceArray [copy] { choices: [MixedChoice; 2]; }
    data FloatingChoice [copy] { case Empty; case Number(value: f64); }
    data FloatingEnvelope [copy] { choice: FloatingChoice; }
    data BorrowedEnvelope [copy] { choice: Choice; borrowed: &u8; }
    data TextEnvelope [copy] { choice: Choice; text: Text; }
    trait Shape { machine code(&self) -> u8; }
    data DynamicEnvelope [copy] { choice: Choice; shape: dyn Shape; }
    data Carrier [copy] { case Unit; }
    proposition same(left: Carrier, right: Carrier) = left == right;
    data Quotient = Carrier % same;
    data QuotientEnvelope [copy] { choice: Choice; quotient: Quotient; }
    data GenericEnvelope<T [copy]> [copy] { choice: Choice; value: T; }
"#;

fn typed() -> TypedTrees {
    let tokens = Lexer::new(SOURCE).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    lower_symbol_resolved_trees(&resolved).expect("type")
}

fn conventional_sum_layout(typed: &TypedTrees, schema_name: &str) -> ConventionalSumLayoutReport {
    let data = unique_data_by_name(typed, schema_name).expect("sum definition");
    let members = typed.data_members(data);
    let cases = members
        .iter()
        .filter_map(|member| match member {
            DataMember::Variant(variant) => Some(variant),
            DataMember::Field(_) => None,
        })
        .collect::<Vec<_>>();
    let mut maximum_align = 1;
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
                .expect("fixed payload field");
                maximum_align = maximum_align.max(align);
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
                    .expect("fixed common field");
            common_align = common_align.max(align);
            common_end = checked_align_up(common_end, align).expect("common alignment");
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
    let common_end = checked_align_up(common_end, common_align)
        .expect("common extent")
        .max(4);
    let payload_base = checked_align_up(common_end, maximum_align).expect("payload base");
    let mut maximum_end = common_end;
    let cases = cases
        .iter()
        .zip(shapes)
        .enumerate()
        .map(|(ordinal, (case, fields))| {
            let mut offset = payload_base;
            let payload_fields = fields
                .into_iter()
                .map(|(field, size, align)| {
                    offset = checked_align_up(offset, align).expect("payload alignment");
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
            maximum_end = maximum_end.max(offset);
            ConventionalSumCaseLayoutReport {
                case: case.name.to_string(),
                member_identity: case.identity,
                ordinal: ordinal as u32,
                payload_fields,
            }
        })
        .collect();
    let align = 4.max(common_align).max(maximum_align);
    ConventionalSumLayoutReport {
        schema_report_fingerprint: normalized_schema_report_fingerprint(typed, data),
        tag_offset: 0,
        tag_size: 4,
        tag_align: 4,
        common_fields,
        cases,
        size: checked_align_up(maximum_end, align).expect("sum extent"),
        align,
    }
}

fn outer_layout(
    typed: &TypedTrees,
    schema_name: &str,
    offsets: &[u64],
    size: u64,
    align: u64,
) -> LayoutPlanReport {
    let data = unique_data_by_name(typed, schema_name).expect("record definition");
    let fields = typed
        .data_members(data)
        .iter()
        .filter_map(|member| match member {
            DataMember::Field(field) if !field.relevance.is_erased() => Some(field),
            DataMember::Field(_) | DataMember::Variant(_) => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(fields.len(), offsets.len());
    LayoutPlanReport {
        schema_report_fingerprint: normalized_schema_report_fingerprint(typed, data),
        entries: fields
            .iter()
            .zip(offsets)
            .map(|(field, offset)| LayoutFieldEntryReport {
                field: field.name.to_string(),
                member_identity: field.identity,
                placement: LayoutPlacementReport::At { offset: *offset },
            })
            .collect(),
        offsets: Some(offsets.to_vec()),
        size: Some(size),
        align,
    }
}

fn choice_value() -> BuildTimeValue {
    BuildTimeValue::Case {
        variant: "Wide".into(),
        payload: vec![
            ("code".into(), BuildTimeValue::Int(0x1122_3344)),
            ("flag".into(), BuildTimeValue::Int(9)),
        ],
    }
}

fn small_choice_value(value: i64) -> BuildTimeValue {
    BuildTimeValue::Case {
        variant: "Small".into(),
        payload: vec![("value".into(), BuildTimeValue::Int(value))],
    }
}

fn direct_sum_rows(
    outer: &LayoutPlanReport,
    rows: Vec<(&str, ConventionalSumLayoutReport)>,
) -> Vec<ConventionalSumFieldLayoutReport> {
    rows.into_iter()
        .map(|(field, layout)| {
            let entry = outer
                .entries
                .iter()
                .find(|entry| entry.field == field)
                .expect("direct sum field has one outer row");
            ConventionalSumFieldLayoutReport {
                field: field.into(),
                member_identity: entry.member_identity,
                layout,
            }
        })
        .collect()
}

fn sum_array_row(
    outer: &LayoutPlanReport,
    field: &str,
    element_count: u64,
    element_layout: ConventionalSumLayoutReport,
) -> ConventionalSumArrayFieldLayoutReport {
    let entry = outer
        .entries
        .iter()
        .find(|entry| entry.field == field)
        .expect("sum array field has one outer row");
    ConventionalSumArrayFieldLayoutReport {
        field: field.into(),
        member_identity: entry.member_identity,
        element_count,
        element_stride: element_layout.size,
        element_layout,
    }
}

fn envelope_value() -> BuildTimeValue {
    BuildTimeValue::Struct {
        type_name: "Envelope".into(),
        fields: vec![
            ("prefix".into(), BuildTimeValue::Int(7)),
            ("choice".into(), choice_value()),
            ("suffix".into(), BuildTimeValue::Int(0x5566)),
        ],
    }
}

#[test]
fn one_nested_sum_retains_both_layouts_selection_byte_order_and_zero_padding() {
    let typed = typed();
    let nested = conventional_sum_layout(&typed, "Choice");
    let outer = outer_layout(&typed, "Envelope", &[0, 4, 18], 20, 4);
    let value = envelope_value();

    let little = validate_const_materializable_record_with_conventional_sum(
        &typed,
        "Envelope",
        &outer,
        &nested,
        &value,
        ByteOrder::LittleEndian,
    )
    .expect("one direct pure-sum field should materialize");
    assert_eq!(little.nested_sum_field(), "choice");
    assert_eq!(little.nested_sum().schema_name(), "Choice");
    assert_eq!(little.nested_sum().selected_case_ordinal(), 2);
    assert_ne!(little.non_authoritative_schema_report_fingerprint(), 0);
    assert_ne!(little.non_authoritative_layout_report_fingerprint(), 0);
    assert_ne!(
        little.non_authoritative_materialization_report_fingerprint(),
        0
    );
    assert_eq!(
        little.bytes(),
        &[
            7, 0, 0, 0, 2, 0, 0, 0, 0x44, 0x33, 0x22, 0x11, 9, 0, 0, 0, 0, 0, 0x66, 0x55,
        ]
    );
    little
        .replay_against(
            &typed,
            "Envelope",
            &outer,
            &nested,
            &value,
            ByteOrder::LittleEndian,
        )
        .expect("both exact layouts replay");

    let big = validate_const_materializable_record_with_conventional_sum(
        &typed,
        "Envelope",
        &outer,
        &nested,
        &value,
        ByteOrder::BigEndian,
    )
    .expect("target byte order remains explicit");
    assert_eq!(
        big.bytes(),
        &[
            7, 0, 0, 0, 0, 0, 0, 2, 0x11, 0x22, 0x33, 0x44, 9, 0, 0, 0, 0, 0, 0x55, 0x66,
        ]
    );
    assert_ne!(
        little.non_authoritative_materialization_report_fingerprint(),
        big.non_authoritative_materialization_report_fingerprint()
    );

    let mut destination = [0xa5; 24];
    little
        .apply(&typed, &mut destination)
        .expect("exact evidence copies atomically");
    assert_eq!(&destination[..20], little.bytes());
    assert_eq!(&destination[20..], &[0xa5; 4]);
}

#[test]
fn replay_rejects_outer_nested_selection_byte_and_compact_coordinate_drift_atomically() {
    let typed = typed();
    let nested = conventional_sum_layout(&typed, "Choice");
    let outer = outer_layout(&typed, "Envelope", &[0, 4, 18], 20, 4);
    let value = envelope_value();
    let carrier = validate_const_materializable_record_with_conventional_sum(
        &typed,
        "Envelope",
        &outer,
        &nested,
        &value,
        ByteOrder::LittleEndian,
    )
    .expect("fixture should validate");

    let mut wrong_outer = outer.clone();
    wrong_outer.entries[2].placement = LayoutPlacementReport::At { offset: 16 };
    assert!(
        carrier
            .replay_against(
                &typed,
                "Envelope",
                &wrong_outer,
                &nested,
                &value,
                ByteOrder::LittleEndian,
            )
            .is_err()
    );

    let mut wrong_nested = nested.clone();
    wrong_nested.cases[2].payload_fields[0].offset += 1;
    assert!(
        carrier
            .replay_against(
                &typed,
                "Envelope",
                &outer,
                &wrong_nested,
                &value,
                ByteOrder::LittleEndian,
            )
            .is_err()
    );

    let mut wrong_value = value.clone();
    let BuildTimeValue::Struct { fields, .. } = &mut wrong_value else {
        unreachable!("fixture is a record")
    };
    fields[1].1 = BuildTimeValue::Case {
        variant: "Empty".into(),
        payload: Vec::new(),
    };
    assert!(
        carrier
            .replay_against(
                &typed,
                "Envelope",
                &outer,
                &nested,
                &wrong_value,
                ByteOrder::LittleEndian,
            )
            .is_err()
    );
    assert!(
        carrier
            .replay_against(
                &typed,
                "Envelope",
                &outer,
                &nested,
                &value,
                ByteOrder::BigEndian,
            )
            .is_err()
    );

    let mut short = [0xa5; 19];
    assert!(carrier.apply(&typed, &mut short).is_err());
    assert_eq!(short, [0xa5; 19]);

    let mut corrupted = carrier;
    corrupted.bytes[12] ^= 1;
    let mut unchanged = [0x5a; 20];
    assert!(corrupted.apply(&typed, &mut unchanged).is_err());
    assert_eq!(unchanged, [0x5a; 20]);

    let mut compact_equal = validate_const_materializable_record_with_conventional_sum(
        &typed,
        "Envelope",
        &outer,
        &nested,
        &value,
        ByteOrder::LittleEndian,
    )
    .expect("second fixture should validate");
    compact_equal.non_authoritative_layout_report_fingerprint =
        normalized_layout_plan_report_fingerprint(&wrong_outer);
    let error = compact_equal
        .replay_against(
            &typed,
            "Envelope",
            &wrong_outer,
            &nested,
            &value,
            ByteOrder::LittleEndian,
        )
        .expect_err("compact-equal outer substitution must reject");
    assert!(error.0.contains("outer layout drifted"));
}

#[test]
fn multiple_direct_sums_retain_complete_ordered_occurrences_and_reject_row_drift() {
    let typed = typed();
    let choice = conventional_sum_layout(&typed, "Choice");
    let outer = outer_layout(&typed, "TwoChoices", &[0, 12], 24, 4);
    let rows = direct_sum_rows(
        &outer,
        vec![("first", choice.clone()), ("second", choice.clone())],
    );
    let value = BuildTimeValue::Struct {
        type_name: "TwoChoices".into(),
        fields: vec![
            ("first".into(), small_choice_value(0x1122)),
            ("second".into(), choice_value()),
        ],
    };
    let carrier = validate_const_materializable_record_with_conventional_sums(
        &typed,
        "TwoChoices",
        &outer,
        &rows,
        &value,
        ByteOrder::LittleEndian,
    )
    .expect("two occurrences of the same sum type should materialize independently");
    assert_eq!(
        carrier
            .nested_sums()
            .iter()
            .map(|row| (row.field(), row.nested_sum().selected_case_ordinal()))
            .collect::<Vec<_>>(),
        [("first", 1), ("second", 2)]
    );
    assert_eq!(
        carrier.bytes(),
        &[
            1, 0, 0, 0, 0x22, 0x11, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0x44, 0x33, 0x22, 0x11, 9, 0, 0,
            0,
        ]
    );
    carrier
        .replay_against_sum_fields(
            &typed,
            "TwoChoices",
            &outer,
            &rows,
            &value,
            ByteOrder::LittleEndian,
        )
        .expect("the complete field occurrence set should replay");

    let mut missing = rows.clone();
    missing.pop();
    let mut extra = rows.clone();
    extra.push(rows[0].clone());
    let mut reordered = rows.clone();
    reordered.swap(0, 1);
    let duplicate = vec![rows[0].clone(), rows[0].clone()];
    let mut wrong_field_identity = rows.clone();
    wrong_field_identity[1].member_identity = Some(99);
    for (name, changed) in [
        ("missing", missing),
        ("extra", extra),
        ("reordered", reordered),
        ("duplicate", duplicate),
        ("field identity", wrong_field_identity),
    ] {
        assert!(
            carrier
                .replay_against_sum_fields(
                    &typed,
                    "TwoChoices",
                    &outer,
                    &changed,
                    &value,
                    ByteOrder::LittleEndian,
                )
                .is_err(),
            "{name} field rows must reject"
        );
    }

    let mut wrong_layout = rows.clone();
    wrong_layout[1].layout.size += 4;
    let mut wrong_case = rows.clone();
    wrong_case[1].layout.cases[2].ordinal = 1;
    let mut wrong_offset = rows.clone();
    wrong_offset[1].layout.cases[2].payload_fields[0].offset += 1;
    for (name, changed) in [
        ("layout", wrong_layout),
        ("case", wrong_case),
        ("offset", wrong_offset),
    ] {
        assert!(
            carrier
                .replay_against_sum_fields(
                    &typed,
                    "TwoChoices",
                    &outer,
                    &changed,
                    &value,
                    ByteOrder::LittleEndian,
                )
                .is_err(),
            "per-field {name} drift must reject"
        );
    }
    assert!(
        carrier
            .replay_against_sum_fields(
                &typed,
                "TwoChoices",
                &outer,
                &rows,
                &value,
                ByteOrder::BigEndian,
            )
            .is_err()
    );

    let mut short = [0xa5; 23];
    assert!(carrier.apply(&typed, &mut short).is_err());
    assert_eq!(short, [0xa5; 23]);
}

#[test]
fn one_sum_array_retains_each_index_and_atomically_materializes_different_cases() {
    let typed = typed();
    let choice = conventional_sum_layout(&typed, "Choice");
    let outer = outer_layout(&typed, "ChoiceArray", &[0], 24, 4);
    let row = sum_array_row(&outer, "choices", 2, choice);
    let value = BuildTimeValue::Struct {
        type_name: "ChoiceArray".into(),
        fields: vec![(
            "choices".into(),
            BuildTimeValue::Array(vec![small_choice_value(0x1122), choice_value()]),
        )],
    };
    let carrier = validate_const_materializable_record_with_conventional_sum_array(
        &typed,
        "ChoiceArray",
        &outer,
        &row,
        &value,
        ByteOrder::LittleEndian,
    )
    .expect("the sole direct fixed array should retain each selected sum independently");
    assert_eq!(
        carrier
            .elements()
            .iter()
            .map(|element| {
                (
                    element.literal_index(),
                    element.nested_sum().selected_case_ordinal(),
                )
            })
            .collect::<Vec<_>>(),
        [(0, 1), (1, 2)]
    );
    assert_eq!(
        carrier.bytes(),
        &[
            1, 0, 0, 0, 0x22, 0x11, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0x44, 0x33, 0x22, 0x11, 9, 0, 0,
            0,
        ]
    );
    carrier
        .replay_against(
            &typed,
            "ChoiceArray",
            &outer,
            &row,
            &value,
            ByteOrder::LittleEndian,
        )
        .expect("the compact layout and every indexed selection should replay");
    let mut destination = [0xa5; 28];
    carrier
        .apply(&typed, &mut destination)
        .expect("the complete outer image should copy atomically");
    assert_eq!(&destination[..24], carrier.bytes());
    assert_eq!(&destination[24..], &[0xa5; 4]);

    let big = validate_const_materializable_record_with_conventional_sum_array(
        &typed,
        "ChoiceArray",
        &outer,
        &row,
        &value,
        ByteOrder::BigEndian,
    )
    .expect("indexed staging retains explicit target byte order");
    assert_eq!(
        big.bytes(),
        &[
            0, 0, 0, 1, 0x11, 0x22, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0x11, 0x22, 0x33, 0x44, 9, 0, 0,
            0,
        ]
    );
    assert_ne!(
        carrier.non_authoritative_materialization_report_fingerprint(),
        big.non_authoritative_materialization_report_fingerprint()
    );

    let mut wrong_count = row.clone();
    wrong_count.element_count = 1;
    let mut wrong_stride = row.clone();
    wrong_stride.element_stride += 4;
    let mut wrong_field = row.clone();
    wrong_field.field = "other".into();
    let mut wrong_layout = row.clone();
    wrong_layout.element_layout.cases[2].payload_fields[0].offset += 1;
    for (name, changed) in [
        ("count", wrong_count),
        ("stride", wrong_stride),
        ("field", wrong_field),
        ("layout", wrong_layout),
    ] {
        assert!(
            carrier
                .replay_against(
                    &typed,
                    "ChoiceArray",
                    &outer,
                    &changed,
                    &value,
                    ByteOrder::LittleEndian,
                )
                .is_err(),
            "{name} drift must reject"
        );
    }

    let mut short = [0x5a; 23];
    assert!(carrier.apply(&typed, &mut short).is_err());
    assert_eq!(short, [0x5a; 23]);
    let mut corrupted = carrier;
    corrupted.elements[1].literal_index = 0;
    let mut unchanged = [0x3c; 24];
    assert!(corrupted.apply(&typed, &mut unchanged).is_err());
    assert_eq!(unchanged, [0x3c; 24]);
}

#[test]
fn multiple_sum_arrays_retain_authored_fields_and_reject_row_set_drift() {
    let typed = typed();
    let choice = conventional_sum_layout(&typed, "Choice");
    let outer = outer_layout(&typed, "TwoChoiceArrays", &[0, 12], 36, 4);
    let rows = vec![
        sum_array_row(&outer, "first", 1, choice.clone()),
        sum_array_row(&outer, "second", 2, choice),
    ];
    let value = BuildTimeValue::Struct {
        type_name: "TwoChoiceArrays".into(),
        fields: vec![
            (
                "first".into(),
                BuildTimeValue::Array(vec![small_choice_value(0x1122)]),
            ),
            (
                "second".into(),
                BuildTimeValue::Array(vec![
                    choice_value(),
                    BuildTimeValue::Case {
                        variant: "Empty".into(),
                        payload: Vec::new(),
                    },
                ]),
            ),
        ],
    };
    let carrier = validate_const_materializable_record_with_conventional_sum_arrays(
        &typed,
        "TwoChoiceArrays",
        &outer,
        &rows,
        &value,
        ByteOrder::LittleEndian,
    )
    .expect("the complete authored-order sum-array field set should materialize");
    assert_eq!(
        carrier
            .arrays()
            .iter()
            .map(|array| (
                array.field(),
                array
                    .elements()
                    .iter()
                    .map(|element| element.selected_case_ordinal())
                    .collect::<Vec<_>>()
            ))
            .collect::<Vec<_>>(),
        [("first", vec![1]), ("second", vec![2, 0])]
    );
    assert_eq!(
        carrier.bytes(),
        &[
            1, 0, 0, 0, 0x22, 0x11, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0x44, 0x33, 0x22, 0x11, 9, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ]
    );
    carrier
        .replay_against(
            &typed,
            "TwoChoiceArrays",
            &outer,
            &rows,
            &value,
            ByteOrder::LittleEndian,
        )
        .expect("the complete row set should replay");
    assert!(
        carrier
            .replay_against(
                &typed,
                "TwoChoiceArrays",
                &outer,
                &rows,
                &value,
                ByteOrder::BigEndian,
            )
            .is_err(),
        "byte-order drift must reject"
    );
    let big = validate_const_materializable_record_with_conventional_sum_arrays(
        &typed,
        "TwoChoiceArrays",
        &outer,
        &rows,
        &value,
        ByteOrder::BigEndian,
    )
    .expect("plural indexed custody should stage big-endian sums");
    assert_eq!(
        big.bytes(),
        &[
            0, 0, 0, 1, 0x11, 0x22, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0x11, 0x22, 0x33, 0x44, 9, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ]
    );

    let mut destination = [0xcc; 40];
    carrier
        .apply(&typed, &mut destination)
        .expect("plural custody should atomically copy the complete image");
    assert_eq!(&destination[..36], carrier.bytes());
    assert_eq!(&destination[36..], &[0xcc; 4]);

    let rejects = |changed: &[ConventionalSumArrayFieldLayoutReport]| {
        assert!(
            carrier
                .replay_against(
                    &typed,
                    "TwoChoiceArrays",
                    &outer,
                    changed,
                    &value,
                    ByteOrder::LittleEndian,
                )
                .is_err()
        );
    };
    rejects(&rows[..1]);
    let mut extra = rows.clone();
    extra.push(rows[0].clone());
    rejects(&extra);
    let mut reordered = rows.clone();
    reordered.swap(0, 1);
    rejects(&reordered);
    let mut duplicate = rows.clone();
    duplicate[1] = rows[0].clone();
    rejects(&duplicate);
    let mut wrong_count = rows.clone();
    wrong_count[1].element_count = 1;
    rejects(&wrong_count);
    let mut wrong_stride = rows.clone();
    wrong_stride[0].element_stride += 4;
    rejects(&wrong_stride);
    let mut wrong_layout = rows.clone();
    wrong_layout[1].element_layout.cases[2].payload_fields[0].offset += 1;
    rejects(&wrong_layout);

    let mut short = [0xa5; 35];
    assert!(carrier.apply(&typed, &mut short).is_err());
    assert_eq!(short, [0xa5; 35]);

    let fresh = || {
        validate_const_materializable_record_with_conventional_sum_arrays(
            &typed,
            "TwoChoiceArrays",
            &outer,
            &rows,
            &value,
            ByteOrder::LittleEndian,
        )
        .expect("fresh plural custody")
    };
    let mut corrupted_index = fresh();
    corrupted_index.arrays[1].elements[1].literal_index = 0;
    let mut unchanged = [0x6d; 36];
    assert!(corrupted_index.apply(&typed, &mut unchanged).is_err());
    assert_eq!(unchanged, [0x6d; 36]);

    let mut corrupted_case = fresh();
    corrupted_case.arrays[1].elements[1].selected_case_ordinal = 1;
    let mut unchanged = [0x7e; 36];
    assert!(corrupted_case.apply(&typed, &mut unchanged).is_err());
    assert_eq!(unchanged, [0x7e; 36]);

    let mut corrupted_element_bytes = fresh();
    corrupted_element_bytes.arrays[1].elements[1].bytes[0] = 1;
    let mut unchanged = [0x8f; 36];
    assert!(
        corrupted_element_bytes
            .apply(&typed, &mut unchanged)
            .is_err()
    );
    assert_eq!(unchanged, [0x8f; 36]);
}

#[test]
fn zero_multiple_nested_and_recursive_sum_shapes_remain_fenced() {
    let typed = typed();
    let nested = conventional_sum_layout(&typed, "Choice");

    let cases = [(
        "DeepEnvelope",
        BuildTimeValue::Struct {
            type_name: "DeepEnvelope".into(),
            fields: vec![(
                "inner".into(),
                BuildTimeValue::Struct {
                    type_name: "InnerEnvelope".into(),
                    fields: vec![("choice".into(), choice_value())],
                },
            )],
        },
        vec![0],
        12,
        "sum",
    )];
    for (schema, value, offsets, size, expected) in cases {
        let layout = outer_layout(&typed, schema, &offsets, size, 4);
        let error = validate_const_materializable_record_with_conventional_sum(
            &typed,
            schema,
            &layout,
            &nested,
            &value,
            ByteOrder::LittleEndian,
        )
        .expect_err("broader nested-sum shape must reject");
        assert!(error.0.contains(expected), "{schema}: {error:?}");
    }

    for (schema, array_field, value, offsets, size) in [
        (
            "ZeroChoiceArray",
            "choices",
            BuildTimeValue::Struct {
                type_name: "ZeroChoiceArray".into(),
                fields: vec![("choices".into(), BuildTimeValue::Array(Vec::new()))],
            },
            vec![0],
            0,
        ),
        (
            "TwoChoiceArrays",
            "first",
            BuildTimeValue::Struct {
                type_name: "TwoChoiceArrays".into(),
                fields: vec![
                    ("first".into(), BuildTimeValue::Array(vec![choice_value()])),
                    ("second".into(), BuildTimeValue::Array(vec![choice_value()])),
                ],
            },
            vec![0, 12],
            24,
        ),
        (
            "NestedChoiceArray",
            "choices",
            BuildTimeValue::Struct {
                type_name: "NestedChoiceArray".into(),
                fields: vec![(
                    "choices".into(),
                    BuildTimeValue::Array(vec![BuildTimeValue::Array(vec![
                        choice_value(),
                        choice_value(),
                    ])]),
                )],
            },
            vec![0],
            24,
        ),
    ] {
        let outer = outer_layout(&typed, schema, &offsets, size, 4);
        let row = sum_array_row(&outer, array_field, 1, nested.clone());
        assert!(
            validate_const_materializable_record_with_conventional_sum_array(
                &typed,
                schema,
                &outer,
                &row,
                &value,
                ByteOrder::LittleEndian,
            )
            .is_err(),
            "{schema} must remain outside the first sum-array rung"
        );
    }

    let direct_and_nested_outer =
        outer_layout(&typed, "DirectAndNestedChoiceArray", &[0, 12], 24, 4);
    let direct_and_nested_row =
        sum_array_row(&direct_and_nested_outer, "direct", 1, nested.clone());
    let direct_and_nested_value = BuildTimeValue::Struct {
        type_name: "DirectAndNestedChoiceArray".into(),
        fields: vec![
            ("direct".into(), BuildTimeValue::Array(vec![choice_value()])),
            (
                "nested".into(),
                BuildTimeValue::Array(vec![BuildTimeValue::Array(vec![choice_value()])]),
            ),
        ],
    };
    assert!(
        validate_const_materializable_record_with_conventional_sum_arrays(
            &typed,
            "DirectAndNestedChoiceArray",
            &direct_and_nested_outer,
            std::slice::from_ref(&direct_and_nested_row),
            &direct_and_nested_value,
            ByteOrder::LittleEndian,
        )
        .is_err(),
        "a nested sum occurrence must not ride beside a direct sum-array field"
    );

    // A record holding a mixed common-field/case member now materializes: the
    // mixed member's value spells its common field inside the merged case
    // payload beside the selected case's own members.
    let mixed_inner = conventional_sum_layout(&typed, "MixedChoice");
    assert_eq!(mixed_inner.common_fields.len(), 1);
    assert_eq!(mixed_inner.common_fields[0].offset, 4);
    assert_eq!(mixed_inner.size, 8);
    let mixed_outer = outer_layout(&typed, "MixedEnvelope", &[0], 8, 4);
    let mixed_value = BuildTimeValue::Struct {
        type_name: "MixedEnvelope".into(),
        fields: vec![(
            "choice".into(),
            BuildTimeValue::Case {
                variant: "Number".into(),
                payload: vec![
                    ("common".into(), BuildTimeValue::Int(9)),
                    ("value".into(), BuildTimeValue::Int(3)),
                ],
            },
        )],
    };
    let carrier = validate_const_materializable_record_with_conventional_sum(
        &typed,
        "MixedEnvelope",
        &mixed_outer,
        &mixed_inner,
        &mixed_value,
        ByteOrder::LittleEndian,
    )
    .expect("a direct mixed member materializes under the same conventional rung");
    assert_eq!(carrier.bytes(), &[1, 0, 0, 0, 9, 3, 0, 0]);

    // A mixed inner layout drifted from its declared common-field geometry
    // still rejects.
    let mut drifted_inner = mixed_inner.clone();
    drifted_inner.common_fields[0].offset = 5;
    assert!(
        validate_const_materializable_record_with_conventional_sum(
            &typed,
            "MixedEnvelope",
            &mixed_outer,
            &drifted_inner,
            &mixed_value,
            ByteOrder::LittleEndian,
        )
        .is_err(),
        "drifted mixed common geometry must reject"
    );
}

#[test]
fn mixed_sum_array_materializes_common_field_and_case_payload_per_element() {
    let typed = typed();
    let element = conventional_sum_layout(&typed, "MixedChoice");
    let outer = outer_layout(&typed, "MixedChoiceArray", &[0], 16, 4);
    let row = sum_array_row(&outer, "choices", 2, element);
    let value = BuildTimeValue::Struct {
        type_name: "MixedChoiceArray".into(),
        fields: vec![(
            "choices".into(),
            BuildTimeValue::Array(vec![
                BuildTimeValue::Case {
                    variant: "Number".into(),
                    payload: vec![
                        ("common".into(), BuildTimeValue::Int(9)),
                        ("value".into(), BuildTimeValue::Int(3)),
                    ],
                },
                BuildTimeValue::Case {
                    variant: "Empty".into(),
                    payload: vec![("common".into(), BuildTimeValue::Int(7))],
                },
            ]),
        )],
    };
    let carrier = validate_const_materializable_record_with_conventional_sum_arrays(
        &typed,
        "MixedChoiceArray",
        &outer,
        &[row],
        &value,
        ByteOrder::LittleEndian,
    )
    .expect("a mixed-element sum array materializes under the same compact row");
    assert_eq!(
        carrier.bytes(),
        &[
            // element 0: tag 1, common 9, Number.value 3
            1, 0, 0, 0, 9, 3, 0, 0, // element 1: tag 0, common 7, empty overlay
            0, 0, 0, 0, 7, 0, 0, 0,
        ]
    );
}

#[test]
fn nan_reference_text_dynamic_quotient_and_generic_shapes_remain_fenced() {
    let typed = typed();

    let floating_nested = conventional_sum_layout(&typed, "FloatingChoice");
    let floating_outer = outer_layout(&typed, "FloatingEnvelope", &[0], 16, 8);
    let floating_error = validate_const_materializable_record_with_conventional_sum(
        &typed,
        "FloatingEnvelope",
        &floating_outer,
        &floating_nested,
        &BuildTimeValue::Struct {
            type_name: "FloatingEnvelope".into(),
            fields: vec![(
                "choice".into(),
                BuildTimeValue::Case {
                    variant: "Number".into(),
                    payload: vec![("value".into(), BuildTimeValue::Float(f64::NAN))],
                },
            )],
        },
        ByteOrder::LittleEndian,
    )
    .expect_err("NaN remains fenced");
    assert!(floating_error.0.contains("exact raw-NaN"));

    let nested = conventional_sum_layout(&typed, "Choice");
    let unsupported = [
        (
            "BorrowedEnvelope",
            BuildTimeValue::Int(1),
            "reference",
            24,
            8,
        ),
        (
            "TextEnvelope",
            BuildTimeValue::Text(vec![1, 2, 3]),
            "Text",
            24,
            4,
        ),
        ("DynamicEnvelope", BuildTimeValue::Int(1), "dynamic", 24, 8),
        (
            "QuotientEnvelope",
            BuildTimeValue::Case {
                variant: "Unit".into(),
                payload: Vec::new(),
            },
            "quotient",
            16,
            4,
        ),
    ];
    for (schema, unsupported_value, expected, size, align) in unsupported {
        let layout = outer_layout(&typed, schema, &[0, 12], size, align);
        let value = BuildTimeValue::Struct {
            type_name: schema.into(),
            fields: vec![
                ("choice".into(), choice_value()),
                (
                    match schema {
                        "BorrowedEnvelope" => "borrowed",
                        "TextEnvelope" => "text",
                        "DynamicEnvelope" => "shape",
                        "QuotientEnvelope" => "quotient",
                        _ => unreachable!(),
                    }
                    .into(),
                    unsupported_value,
                ),
            ],
        };
        let error = validate_const_materializable_record_with_conventional_sum(
            &typed,
            schema,
            &layout,
            &nested,
            &value,
            ByteOrder::LittleEndian,
        )
        .expect_err("unsupported leaf must remain fenced");
        assert!(error.0.contains(expected), "{schema}: {error:?}");
    }

    let generic_data = unique_data_by_name(&typed, "GenericEnvelope").unwrap();
    let generic_layout = LayoutPlanReport {
        schema_report_fingerprint: normalized_schema_report_fingerprint(&typed, generic_data),
        entries: Vec::new(),
        offsets: Some(Vec::new()),
        size: Some(0),
        align: 1,
    };
    let error = validate_const_materializable_record_with_conventional_sum(
        &typed,
        "GenericEnvelope",
        &generic_layout,
        &nested,
        &BuildTimeValue::Struct {
            type_name: "GenericEnvelope".into(),
            fields: vec![
                ("choice".into(), choice_value()),
                ("value".into(), BuildTimeValue::Int(1)),
            ],
        },
        ByteOrder::LittleEndian,
    )
    .expect_err("generic owner remains fenced");
    assert!(error.0.contains("generic, opaque, quotient"));
}
