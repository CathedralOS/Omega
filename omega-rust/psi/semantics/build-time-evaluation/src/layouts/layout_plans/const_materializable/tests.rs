//! Const materializable layout plan tests.

use super::{
    BuildTimeValue, ByteOrder, LayoutPlanReport, TypedTrees,
    normalized_layout_plan_report_fingerprint, normalized_schema_report_fingerprint,
    unique_data_by_name, validate_const_materializable_typed_owned_layout,
};
use layout_plans::{LayoutFieldEntryReport, LayoutPlacementReport};
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::pre_resolution::{
    GenericDataRequest, normalize_generic_data,
};
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
use tokens_to_syntax_trees::parse_syntax_trees;

use crate::layouts::layout_plans::schema_fields;

const SOURCE: &str = r#"
        data Inner [copy] { enabled: bool; code: u32; }
        data Sample [copy] { tag: u8; inner: Inner; words: [u16; 2]; }
        data Floating [copy] { value: f64; }
        data Floating32 [copy] { value: f32; }
        data FloatSample [copy] {
            narrow: f32;
            wide: f64;
            signed_zero: f64;
            infinite: f32;
        }
        data Choice [copy] { case Number(value: u8); case Empty; }
        data Borrowed [copy] { value: &u8; }
    "#;

const UNSUPPORTED_SOURCE: &str = r#"
        boundary data Opaque;
        data Generic<T [copy]> [copy] { value: T; }
        data Sliced [copy] { values: [u8]; }
        trait Shape { machine code(&self) -> u8; }
        data Dynamic [copy] { value: dyn Shape; }
        data Carrier [copy] { case Unit; }
        proposition same(left: Carrier, right: Carrier) = left == right;
        data Quotient = Carrier % same;
    "#;

#[test]
fn nested_records_and_arrays_replay_exact_bytes_and_zero_padding() {
    let typed = typed(SOURCE);
    let layout = sample_layout(&typed);
    let value = sample_value();

    let little = validate_const_materializable_typed_owned_layout(
        &typed,
        "Sample",
        &layout,
        &value,
        ByteOrder::LittleEndian,
    )
    .expect("closed copy record should be ConstMaterializable");
    assert_eq!(
        little.bytes(),
        &[7, 0, 0, 0, 1, 0, 0, 0, 0x44, 0x33, 0x22, 0x11, 2, 1, 4, 3]
    );
    assert_ne!(little.non_authoritative_schema_report_fingerprint(), 0);
    assert_ne!(little.non_authoritative_layout_report_fingerprint(), 0);
    assert_ne!(
        little.non_authoritative_materialization_report_fingerprint(),
        0
    );
    little
        .replay_against(&typed, "Sample", &layout, &value, ByteOrder::LittleEndian)
        .expect("exact inputs replay");

    let big = validate_const_materializable_typed_owned_layout(
        &typed,
        "Sample",
        &layout,
        &value,
        ByteOrder::BigEndian,
    )
    .expect("same value should bind target byte order");
    assert_eq!(
        big.bytes(),
        &[7, 0, 0, 0, 1, 0, 0, 0, 0x11, 0x22, 0x33, 0x44, 1, 2, 3, 4]
    );
    assert_ne!(
        little.non_authoritative_materialization_report_fingerprint(),
        big.non_authoritative_materialization_report_fingerprint()
    );

    let mut destination = [0xa5; 20];
    little
        .apply(&typed, &mut destination)
        .expect("validated evidence copies atomically");
    assert_eq!(&destination[..16], little.bytes());
    assert_eq!(&destination[16..], &[0xa5; 4]);
}

#[test]
fn replay_rejects_every_retained_input_axis_and_preserves_destination() {
    let typed = typed(SOURCE);
    let layout = sample_layout(&typed);
    let value = sample_value();
    let carrier = validate_const_materializable_typed_owned_layout(
        &typed,
        "Sample",
        &layout,
        &value,
        ByteOrder::LittleEndian,
    )
    .expect("fixture should validate");

    let mut wrong_schema = layout.clone();
    wrong_schema.schema_report_fingerprint ^= 1;
    assert!(
        carrier
            .replay_against(
                &typed,
                "Sample",
                &wrong_schema,
                &value,
                ByteOrder::LittleEndian,
            )
            .is_err()
    );

    let mut wrong_member = layout.clone();
    wrong_member.entries[0].member_identity = Some(99);
    assert!(
        carrier
            .replay_against(
                &typed,
                "Sample",
                &wrong_member,
                &value,
                ByteOrder::LittleEndian,
            )
            .is_err()
    );

    let mut wrong_extent = layout.clone();
    wrong_extent.size = Some(17);
    assert!(
        carrier
            .replay_against(
                &typed,
                "Sample",
                &wrong_extent,
                &value,
                ByteOrder::LittleEndian,
            )
            .is_err()
    );

    let mut wrong_offsets = layout.clone();
    wrong_offsets.offsets = Some(vec![0, 5, 12]);
    assert!(
        validate_const_materializable_typed_owned_layout(
            &typed,
            "Sample",
            &wrong_offsets,
            &value,
            ByteOrder::LittleEndian,
        )
        .is_err()
    );

    let mut wrong_alignment = layout.clone();
    wrong_alignment.align = 3;
    assert!(
        validate_const_materializable_typed_owned_layout(
            &typed,
            "Sample",
            &wrong_alignment,
            &value,
            ByteOrder::LittleEndian,
        )
        .is_err()
    );

    let mut wrong_value = value.clone();
    let BuildTimeValue::Struct { fields, .. } = &mut wrong_value else {
        unreachable!("fixture is a record")
    };
    fields[0].1 = BuildTimeValue::Int(8);
    assert!(
        carrier
            .replay_against(
                &typed,
                "Sample",
                &layout,
                &wrong_value,
                ByteOrder::LittleEndian,
            )
            .is_err()
    );
    assert!(
        carrier
            .replay_against(&typed, "Sample", &layout, &value, ByteOrder::BigEndian,)
            .is_err()
    );

    let mut destination = [0x5a; 15];
    carrier
        .apply(&typed, &mut destination)
        .expect_err("short destination rejects");
    assert_eq!(destination, [0x5a; 15]);

    let mut corrupted = carrier;
    corrupted.bytes[1] = 9;
    let mut destination = [0x6b; 16];
    corrupted
        .apply(&typed, &mut destination)
        .expect_err("stored-byte drift rejects before copying");
    assert_eq!(destination, [0x6b; 16]);
}

#[test]
fn replay_rejects_layout_substitution_when_compact_report_fingerprint_is_forced_equal() {
    let typed = typed(SOURCE);
    let layout = sample_layout(&typed);
    let value = sample_value();
    let mut carrier = validate_const_materializable_typed_owned_layout(
        &typed,
        "Sample",
        &layout,
        &value,
        ByteOrder::LittleEndian,
    )
    .expect("fixture should validate");

    let mut substituted_layout = layout.clone();
    substituted_layout.size = Some(17);
    carrier.non_authoritative_layout_report_fingerprint =
        normalized_layout_plan_report_fingerprint(&substituted_layout);

    let error = carrier
        .replay_against(
            &typed,
            "Sample",
            &substituted_layout,
            &value,
            ByteOrder::LittleEndian,
        )
        .expect_err("exact retained layout rejects a compact-equal substitute");
    assert!(error.0.contains("layout drifted"));
}

#[test]
fn non_nan_float_leaves_retain_exact_format_bits_and_byte_order() {
    let typed = typed(SOURCE);
    let layout = layout(&typed, "FloatSample", &[0, 8, 16, 24], 32, 8);
    let value = BuildTimeValue::Struct {
        type_name: "FloatSample".into(),
        fields: vec![
            ("narrow".into(), BuildTimeValue::Float(1.5)),
            ("wide".into(), BuildTimeValue::Float(-3.25)),
            ("signed_zero".into(), BuildTimeValue::Float(-0.0)),
            ("infinite".into(), BuildTimeValue::Float(f64::INFINITY)),
        ],
    };

    let little = validate_const_materializable_typed_owned_layout(
        &typed,
        "FloatSample",
        &layout,
        &value,
        ByteOrder::LittleEndian,
    )
    .expect("non-NaN binary32/binary64 leaves should materialize");
    assert_eq!(
        little.bytes(),
        &[
            0x00, 0x00, 0xc0, 0x3f, 0, 0, 0, 0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x0a, 0xc0,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80, 0x00, 0x00, 0x80, 0x7f, 0, 0, 0, 0,
        ]
    );

    let big = validate_const_materializable_typed_owned_layout(
        &typed,
        "FloatSample",
        &layout,
        &value,
        ByteOrder::BigEndian,
    )
    .expect("target byte order should remain explicit for float leaves");
    assert_eq!(
        big.bytes(),
        &[
            0x3f, 0xc0, 0x00, 0x00, 0, 0, 0, 0, 0xc0, 0x0a, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x7f, 0x80, 0x00, 0x00, 0, 0, 0, 0,
        ]
    );
    assert_ne!(
        little.non_authoritative_materialization_report_fingerprint(),
        big.non_authoritative_materialization_report_fingerprint()
    );
    little
        .replay_against(
            &typed,
            "FloatSample",
            &layout,
            &value,
            ByteOrder::LittleEndian,
        )
        .expect("exact float custody should replay");
}

#[test]
fn unsupported_or_malformed_value_shapes_fail_closed() {
    let typed = typed(SOURCE);

    let float_layout = one_field_layout(&typed, "Floating", 8, 8);
    let float = BuildTimeValue::Struct {
        type_name: "Floating".into(),
        fields: vec![("value".into(), BuildTimeValue::Float(f64::NAN))],
    };
    let error = validate_const_materializable_typed_owned_layout(
        &typed,
        "Floating",
        &float_layout,
        &float,
        ByteOrder::LittleEndian,
    )
    .expect_err("NaN without exact raw representation evidence remains fenced");
    assert!(error.0.contains("exact raw-NaN realization"), "{error:?}");

    let narrow_layout = one_field_layout(&typed, "Floating32", 4, 4);
    let rounded = BuildTimeValue::Struct {
        type_name: "Floating32".into(),
        fields: vec![("value".into(), BuildTimeValue::Float(0.1))],
    };
    let error = validate_const_materializable_typed_owned_layout(
        &typed,
        "Floating32",
        &narrow_layout,
        &rounded,
        ByteOrder::LittleEndian,
    )
    .expect_err("a forged f64 value cannot acquire binary32 custody by rounding");
    assert!(error.0.contains("exact binary32 value"), "{error:?}");

    let choice = BuildTimeValue::Case {
        variant: "Empty".into(),
        payload: Vec::new(),
    };
    let choice_data = unique_data_by_name(&typed, "Choice").expect("choice");
    let choice_layout = LayoutPlanReport {
        schema_report_fingerprint: normalized_schema_report_fingerprint(&typed, choice_data),
        entries: Vec::new(),
        offsets: Some(Vec::new()),
        size: Some(0),
        align: 1,
    };
    assert!(
        validate_const_materializable_typed_owned_layout(
            &typed,
            "Choice",
            &choice_layout,
            &choice,
            ByteOrder::LittleEndian,
        )
        .is_err()
    );

    let borrowed = BuildTimeValue::Struct {
        type_name: "Borrowed".into(),
        fields: vec![("value".into(), BuildTimeValue::Int(1))],
    };
    let borrowed_data = unique_data_by_name(&typed, "Borrowed").expect("borrowed");
    let borrowed_layout = LayoutPlanReport {
        schema_report_fingerprint: normalized_schema_report_fingerprint(&typed, borrowed_data),
        entries: Vec::new(),
        offsets: Some(Vec::new()),
        size: Some(0),
        align: 1,
    };
    let error = validate_const_materializable_typed_owned_layout(
        &typed,
        "Borrowed",
        &borrowed_layout,
        &borrowed,
        ByteOrder::LittleEndian,
    )
    .expect_err("references remain fenced");
    assert!(error.0.contains("reference"), "{error:?}");

    let mut malformed = sample_value();
    let BuildTimeValue::Struct { fields, .. } = &mut malformed else {
        unreachable!("fixture is a record")
    };
    fields[2].1 = BuildTimeValue::Text(vec![1, 2, 3, 4]);
    let error = validate_const_materializable_typed_owned_layout(
        &typed,
        "Sample",
        &sample_layout(&typed),
        &malformed,
        ByteOrder::LittleEndian,
    )
    .expect_err("Text cannot substitute for an array");
    assert!(error.0.contains("contains Text"), "{error:?}");
}

#[test]
fn generic_opaque_slice_dynamic_and_quotient_shapes_fail_closed() {
    let typed = typed(UNSUPPORTED_SOURCE);

    let cases = [
        ("Opaque", BuildTimeValue::Unit, "generic, opaque, quotient"),
        (
            "Generic",
            BuildTimeValue::Struct {
                type_name: "Generic".into(),
                fields: vec![("value".into(), BuildTimeValue::Int(1))],
            },
            "generic, opaque, quotient",
        ),
        (
            "Sliced",
            BuildTimeValue::Struct {
                type_name: "Sliced".into(),
                fields: vec![("values".into(), BuildTimeValue::Array(Vec::new()))],
            },
            "slice",
        ),
        (
            "Dynamic",
            BuildTimeValue::Struct {
                type_name: "Dynamic".into(),
                fields: vec![("value".into(), BuildTimeValue::Unit)],
            },
            "dynamic",
        ),
        (
            "Quotient",
            BuildTimeValue::Case {
                variant: "Unit".into(),
                payload: Vec::new(),
            },
            "generic, opaque, quotient",
        ),
    ];

    for (schema, value, expected) in cases {
        let data = unique_data_by_name(&typed, schema).expect("fixture data");
        let layout = LayoutPlanReport {
            schema_report_fingerprint: normalized_schema_report_fingerprint(&typed, data),
            entries: Vec::new(),
            offsets: Some(Vec::new()),
            size: Some(0),
            align: 1,
        };
        let error = validate_const_materializable_typed_owned_layout(
            &typed,
            schema,
            &layout,
            &value,
            ByteOrder::LittleEndian,
        )
        .expect_err("unsupported shape must reject before byte materialization");
        assert!(error.0.contains(expected), "{schema}: {error:?}");
    }
}

fn typed(source: &str) -> TypedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    lower_symbol_resolved_trees(&resolved).expect("type")
}

fn sample_layout(typed: &TypedTrees) -> LayoutPlanReport {
    layout(typed, "Sample", &[0, 4, 12], 16, 4)
}

fn one_field_layout(typed: &TypedTrees, schema: &str, size: u64, align: u64) -> LayoutPlanReport {
    layout(typed, schema, &[0], size, align)
}

fn layout(
    typed: &TypedTrees,
    schema: &str,
    offsets: &[u64],
    size: u64,
    align: u64,
) -> LayoutPlanReport {
    let (fields, schema_report_fingerprint) = schema_fields(typed, schema).expect("reflect schema");
    assert_eq!(fields.len(), offsets.len());
    LayoutPlanReport {
        schema_report_fingerprint,
        entries: fields
            .iter()
            .zip(offsets)
            .map(|(field, offset)| LayoutFieldEntryReport {
                field: field.name.clone(),
                member_identity: field.identity,
                placement: LayoutPlacementReport::At { offset: *offset },
            })
            .collect(),
        offsets: Some(offsets.to_vec()),
        size: Some(size),
        align,
    }
}

fn sample_value() -> BuildTimeValue {
    BuildTimeValue::Struct {
        type_name: "Sample".into(),
        fields: vec![
            ("tag".into(), BuildTimeValue::Int(7)),
            (
                "inner".into(),
                BuildTimeValue::Struct {
                    type_name: "Inner".into(),
                    fields: vec![
                        ("enabled".into(), BuildTimeValue::Bool(true)),
                        ("code".into(), BuildTimeValue::Int(0x1122_3344)),
                    ],
                },
            ),
            (
                "words".into(),
                BuildTimeValue::Array(vec![
                    BuildTimeValue::Int(0x0102),
                    BuildTimeValue::Int(0x0304),
                ]),
            ),
        ],
    }
}

#[test]
fn closed_generic_instance_members_carry_substituted_literal_arrays() {
    // `Pair<const N>`'s authored `items: [u64; N]` is a non-literal length;
    // `Root.pair`'s `Pair<2>` member reaches the value walk as one closed
    // synthesized record whose substituted member is already literal. The
    // open template stays fenced and the instance encodes at exact offsets.
    let typed = typed_generic(
        "data Pair<const N: u64> [copy] { items: [u64; N]; }
         data Root [copy] { pair: Pair<2>; tail: u8; }",
    );
    let layout = layout(&typed, "Root", &[0, 16], 24, 8);
    let value = BuildTimeValue::Struct {
        type_name: "Root".into(),
        fields: vec![
            (
                "pair".into(),
                BuildTimeValue::Struct {
                    type_name: "Pair<2>".into(),
                    fields: vec![(
                        "items".into(),
                        BuildTimeValue::Array(vec![
                            BuildTimeValue::Int(0x1122_3344_5566_7788),
                            BuildTimeValue::Int(0xcafe),
                        ]),
                    )],
                },
            ),
            ("tail".into(), BuildTimeValue::Int(9)),
        ],
    };
    let materialized = validate_const_materializable_typed_owned_layout(
        &typed,
        "Root",
        &layout,
        &value,
        ByteOrder::LittleEndian,
    )
    .expect("a closed instance member materializes under its exact identity");
    let mut expected = [0_u8; 24];
    expected[0..8].copy_from_slice(&0x1122_3344_5566_7788_u64.to_le_bytes());
    expected[8..16].copy_from_slice(&0xcafe_u64.to_le_bytes());
    expected[16] = 9;
    assert_eq!(materialized.bytes(), expected);

    // The open template itself still has no closed checked-shape identity.
    let template = unique_data_by_name(&typed, "Pair").expect("the open template");
    let open_layout = LayoutPlanReport {
        schema_report_fingerprint: normalized_schema_report_fingerprint(&typed, template),
        entries: Vec::new(),
        offsets: Some(Vec::new()),
        size: Some(0),
        align: 1,
    };
    let error = validate_const_materializable_typed_owned_layout(
        &typed,
        "Pair",
        &open_layout,
        &BuildTimeValue::Struct {
            type_name: "Pair".into(),
            fields: vec![("items".into(), BuildTimeValue::Array(Vec::new()))],
        },
        ByteOrder::LittleEndian,
    )
    .expect_err("the open generic template stays fenced");
    assert!(error.0.contains("generic, opaque, quotient"), "{error:?}");
}

fn typed_generic(source: &str) -> TypedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let syntax =
        normalize_generic_data(GenericDataRequest::new(syntax)).expect("synthesize instances");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    lower_symbol_resolved_trees(&resolved).expect("type")
}
