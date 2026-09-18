use super::write_program;
use build_time_evaluation::{
    BuildTimeValue, compute_layout_plan, evaluate_and_materialize_typed_owned_layout_into,
    materialize_typed_owned_layout_into, validate_const_materializable_typed_owned_layout,
};
use compiler::{CheckedCompileRequest, compile_to_checked};
use layout_plans::{
    AggregateFieldSchema, AggregateFieldValue, ByteOrder, LayoutPlacementReport,
    materialize_aggregate_layout_into,
};

#[test]
fn fixed_records_are_reflected_as_one_nested_at_field() {
    let main_path = write_program(
        "fixed-record-at",
        r#"
use omega::language::core::layout;

data RecordLayout { }
machine RecordLayout::plan(&mut self, schema: Schema) -> Plan {
    let mut entries: [FieldEntry; 64];
    entries[0] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 8 },
    };
    Plan { entries: entries, entry_count: 1,
           size_fixed: 24, size_is_dynamic: false, align: 4 }
}

data Pair { low: u8; high: u32; }
data Samples { pair: Pair; }
data Main { }
machine Main::main(&mut self) { }
"#,
    );
    let checked = compile_to_checked(CheckedCompileRequest::new(&main_path, None))
        .expect("fixed record should reflect");
    let report = compute_layout_plan(&checked.typed, "RecordLayout::plan", "Samples", None)
        .expect("one At placement should admit the complete fixed-record extent");
    assert_eq!(report.offsets, Some(vec![8]));
    assert_eq!(report.size, Some(24));
    assert_eq!(report.align, 4);

    let mut bytes = [0xa5; 24];
    materialize_aggregate_layout_into(
        &report,
        &[AggregateFieldSchema::new("pair", 8).expect("compiler-derived pair extent")],
        &[AggregateFieldValue::new("pair", [1, 0, 0, 0, 5, 4, 3, 2]).expect("owned pair bytes")],
        ByteOrder::LittleEndian,
        &mut bytes,
    )
    .expect("owned fixed record should materialize through its whole At extent");
    assert!(bytes[..8].iter().all(|byte| *byte == 0));
    assert_eq!(&bytes[8..16], &[1, 0, 0, 0, 5, 4, 3, 2]);
    assert!(bytes[16..].iter().all(|byte| *byte == 0));
}

#[test]
fn typed_owned_fixed_records_materialize_without_caller_supplied_field_bytes() {
    let main_path = write_program(
        "typed-owned-fixed-record",
        r#"
use omega::language::core::layout;

data RecordLayout { }
machine RecordLayout::plan(&mut self, schema: Schema) -> Plan {
    let mut entries: [FieldEntry; 64];
    entries[0] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 8 },
    };
    Plan { entries: entries, entry_count: 1,
           size_fixed: 24, size_is_dynamic: false, align: 4 }
}
data Pair { low: u8; high: u32; }
data Samples { pair: Pair; }
data Main { }
machine Main::main(&mut self) { }
"#,
    );
    let checked = compile_to_checked(CheckedCompileRequest::new(&main_path, None))
        .expect("fixed record should reflect");
    let report = compute_layout_plan(&checked.typed, "RecordLayout::plan", "Samples", None)
        .expect("one At placement should admit the typed record extent");
    let value = BuildTimeValue::Struct {
        type_name: "Samples".to_owned(),
        fields: vec![(
            "pair".to_owned(),
            BuildTimeValue::Struct {
                type_name: "Pair".to_owned(),
                fields: vec![
                    ("low".to_owned(), BuildTimeValue::Int(1)),
                    ("high".to_owned(), BuildTimeValue::Int(0x0203_0405)),
                ],
            },
        )],
    };
    let mut bytes = [0xa5; 24];
    materialize_typed_owned_layout_into(
        &checked.typed,
        "Samples",
        &report,
        &value,
        ByteOrder::LittleEndian,
        &mut bytes,
    )
    .expect("typed source-owned record should materialize through compiler-derived bytes");
    assert!(bytes[..8].iter().all(|byte| *byte == 0));
    assert_eq!(&bytes[8..16], &[1, 0, 0, 0, 5, 4, 3, 2]);
    assert!(bytes[16..].iter().all(|byte| *byte == 0));

    let mut big_endian = [0xa5; 24];
    materialize_typed_owned_layout_into(
        &checked.typed,
        "Samples",
        &report,
        &value,
        ByteOrder::BigEndian,
        &mut big_endian,
    )
    .expect("Omega may select a different target byte order at realization");
    assert_eq!(&big_endian[8..16], &[1, 0, 0, 0, 2, 3, 4, 5]);

    let mut wrong_type = value.clone();
    let BuildTimeValue::Struct { type_name, .. } = &mut wrong_type else {
        unreachable!()
    };
    *type_name = "Pair".to_owned();
    let mut unchanged = [0x5a; 24];
    assert!(
        materialize_typed_owned_layout_into(
            &checked.typed,
            "Samples",
            &report,
            &wrong_type,
            ByteOrder::LittleEndian,
            &mut unchanged,
        )
        .is_err()
    );
    assert_eq!(unchanged, [0x5a; 24]);
}

#[test]
fn typed_owned_numbered_aggregate_rejoins_a_retained_layout_after_rename() {
    let legacy_path = write_program(
        "typed-owned-numbered-aggregate-legacy",
        r#"
use omega::language::core::layout;

data RecordLayout { }
machine RecordLayout::plan(&mut self, schema: Schema) -> Plan {
    let mut entries: [FieldEntry; 64];
    entries[0] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 4 },
    };
    Plan { entries: entries, entry_count: 1,
           size_fixed: 12, size_is_dynamic: false, align: 2 }
}
data Pair { low: u16; high: u16; }
data Samples { #7 legacy_pair: Pair; }
data Main { }
machine Main::main(&mut self) { }
"#,
    );
    let legacy = compile_to_checked(CheckedCompileRequest::new(&legacy_path, None))
        .expect("legacy numbered schema should check");
    let retained = compute_layout_plan(&legacy.typed, "RecordLayout::plan", "Samples", None)
        .expect("legacy numbered aggregate layout should validate");
    assert_eq!(retained.entries[0].field, "legacy_pair");
    assert_eq!(retained.entries[0].member_identity, Some(7));

    let renamed_path = write_program(
        "typed-owned-numbered-aggregate-renamed",
        r#"
data Pair { low: u16; high: u16; }
data Samples { #7 pair: Pair; }
data Main { }
machine Main::main(&mut self) { }
"#,
    );
    let renamed = compile_to_checked(CheckedCompileRequest::new(&renamed_path, None))
        .expect("renamed numbered schema should check");
    let value = BuildTimeValue::Struct {
        type_name: "Samples".to_owned(),
        fields: vec![(
            "pair".to_owned(),
            BuildTimeValue::Struct {
                type_name: "Pair".to_owned(),
                fields: vec![
                    ("low".to_owned(), BuildTimeValue::Int(0x0201.into())),
                    ("high".to_owned(), BuildTimeValue::Int(0x0403.into())),
                ],
            },
        )],
    };
    let mut bytes = [0xa5; 12];
    materialize_typed_owned_layout_into(
        &renamed.typed,
        "Samples",
        &retained,
        &value,
        ByteOrder::LittleEndian,
        &mut bytes,
    )
    .expect("stable field identity should rejoin the retained layout after a rename");
    assert_eq!(bytes, [0, 0, 0, 0, 1, 2, 3, 4, 0, 0, 0, 0]);

    let mut drifted = retained;
    drifted.entries[0].member_identity = Some(8);
    let mut unchanged = [0x5a; 12];
    let error = materialize_typed_owned_layout_into(
        &renamed.typed,
        "Samples",
        &drifted,
        &value,
        ByteOrder::LittleEndian,
        &mut unchanged,
    )
    .expect_err("retained member identity drift must reject atomically");
    assert!(error.0.contains("same stable identity"));
    assert_eq!(unchanged, [0x5a; 12]);
}

#[test]
fn source_machine_owned_record_materializes_through_the_typed_bridge() {
    let main_path = write_program(
        "source-owned-fixed-record",
        r#"
use omega::language::core::layout;

data RecordLayout { }
machine RecordLayout::plan(&mut self, schema: Schema) -> Plan {
    let mut entries: [FieldEntry; 64];
    entries[0] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 8 },
    };
    Plan { entries: entries, entry_count: 1,
           size_fixed: 24, size_is_dynamic: false, align: 4 }
}
data Pair { low: u8; high: u32; }
data Samples { pair: Pair; }
machine make_samples() -> Samples {
    Samples { pair: Pair { low: 1, high: 33752069 } }
}
data Main { }
machine Main::main(&mut self) { }
"#,
    );
    let checked = compile_to_checked(CheckedCompileRequest::new(&main_path, None))
        .expect("owned record producer should check");
    let report = compute_layout_plan(&checked.typed, "RecordLayout::plan", "Samples", None)
        .expect("owned record should have one whole-field placement");
    let mut bytes = [0xa5; 24];
    evaluate_and_materialize_typed_owned_layout_into(
        &checked.typed,
        "make_samples",
        "Samples",
        &report,
        ByteOrder::LittleEndian,
        &mut bytes,
    )
    .expect("source-owned record should evaluate and materialize atomically");
    assert_eq!(&bytes[8..16], &[1, 0, 0, 0, 5, 4, 3, 2]);
}

#[test]
fn const_materializable_record_binds_value_layout_order_and_zero_padding() {
    let main_path = write_program(
        "const-materializable-record",
        r#"
use omega::language::core::layout;

data RecordLayout { }
machine RecordLayout::plan(&mut self, schema: Schema) -> Plan {
    let mut entries: [FieldEntry; 64];
    entries[0] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 0 },
    };
    entries[1] = FieldEntry {
        key: schema.fields[1].key,
        placement: FieldPlan::At { offset: 4 },
    };
    entries[2] = FieldEntry {
        key: schema.fields[2].key,
        placement: FieldPlan::At { offset: 12 },
    };
    Plan { entries: entries, entry_count: 3,
           size_fixed: 16, size_is_dynamic: false, align: 4 }
}
data Pair [copy] { enabled: bool; code: u32; }
data Samples [copy] { tag: u8; pair: Pair; words: [u16; 2]; }
machine make_samples() -> Samples {
    Samples {
        tag: 7,
        pair: Pair { enabled: true, code: 287454020 },
        words: [258, 772],
    }
}
data Main { }
machine Main::main(&mut self) { }
"#,
    );
    let checked = compile_to_checked(CheckedCompileRequest::new(&main_path, None))
        .expect("closed copy-valued materialization fixture should check");
    let layout = compute_layout_plan(&checked.typed, "RecordLayout::plan", "Samples", None)
        .expect("fixture layout should validate");
    let value = BuildTimeValue::Struct {
        type_name: "Samples".into(),
        fields: vec![
            ("tag".into(), BuildTimeValue::Int(7)),
            (
                "pair".into(),
                BuildTimeValue::Struct {
                    type_name: "Pair".into(),
                    fields: vec![
                        ("enabled".into(), BuildTimeValue::Bool(true)),
                        ("code".into(), BuildTimeValue::Int(287454020)),
                    ],
                },
            ),
            (
                "words".into(),
                BuildTimeValue::Array(vec![BuildTimeValue::Int(258), BuildTimeValue::Int(772)]),
            ),
        ],
    };

    let little = validate_const_materializable_typed_owned_layout(
        &checked.typed,
        "Samples",
        &layout,
        &value,
        ByteOrder::LittleEndian,
    )
    .expect("supported source value should retain materialization evidence");
    assert_eq!(
        little.bytes(),
        &[7, 0, 0, 0, 1, 0, 0, 0, 0x44, 0x33, 0x22, 0x11, 2, 1, 4, 3]
    );
    let big = validate_const_materializable_typed_owned_layout(
        &checked.typed,
        "Samples",
        &layout,
        &value,
        ByteOrder::BigEndian,
    )
    .expect("target byte order should be selected explicitly");
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
        .apply(&checked.typed, &mut destination)
        .expect("exact replay should copy only the fixed extent");
    assert_eq!(&destination[..16], little.bytes());
    assert_eq!(&destination[16..], &[0xa5; 4]);

    let mut drifted_layout = layout.clone();
    drifted_layout.entries[2].placement = LayoutPlacementReport::At { offset: 11 };
    assert!(
        little
            .replay_against(
                &checked.typed,
                "Samples",
                &drifted_layout,
                &value,
                ByteOrder::LittleEndian,
            )
            .is_err()
    );
    let mut drifted_value = value;
    let BuildTimeValue::Struct { fields, .. } = &mut drifted_value else {
        unreachable!("fixture value is a record")
    };
    fields[0].1 = BuildTimeValue::Int(8);
    assert!(
        little
            .replay_against(
                &checked.typed,
                "Samples",
                &layout,
                &drifted_value,
                ByteOrder::LittleEndian,
            )
            .is_err()
    );
}

#[test]
fn const_materializable_non_nan_float_leaves_bind_exact_format_bits() {
    let main_path = write_program(
        "const-materializable-floats",
        r#"
use omega::language::core::layout;

data FloatLayout { }
machine FloatLayout::plan(&mut self, schema: Schema) -> Plan {
    let mut entries: [FieldEntry; 64];
    entries[0] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 0 },
    };
    entries[1] = FieldEntry {
        key: schema.fields[1].key,
        placement: FieldPlan::At { offset: 8 },
    };
    entries[2] = FieldEntry {
        key: schema.fields[2].key,
        placement: FieldPlan::At { offset: 16 },
    };
    Plan { entries: entries, entry_count: 3,
           size_fixed: 24, size_is_dynamic: false, align: 8 }
}
data Samples [copy] { narrow: f32; wide: f64; signed_zero: f64; }
data Main { }
machine Main::main(&mut self) { }
"#,
    );
    let checked = compile_to_checked(CheckedCompileRequest::new(&main_path, None))
        .expect("closed non-NaN float materialization fixture should check");
    let layout = compute_layout_plan(&checked.typed, "FloatLayout::plan", "Samples", None)
        .expect("fixture layout should validate");
    let value = BuildTimeValue::Struct {
        type_name: "Samples".into(),
        fields: vec![
            ("narrow".into(), BuildTimeValue::Float(1.5)),
            ("wide".into(), BuildTimeValue::Float(f64::INFINITY)),
            ("signed_zero".into(), BuildTimeValue::Float(-0.0)),
        ],
    };

    let little = validate_const_materializable_typed_owned_layout(
        &checked.typed,
        "Samples",
        &layout,
        &value,
        ByteOrder::LittleEndian,
    )
    .expect("non-NaN float leaves should retain exact materialization evidence");
    assert_eq!(
        little.bytes(),
        &[
            0x00, 0x00, 0xc0, 0x3f, 0, 0, 0, 0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xf0, 0x7f,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80,
        ]
    );

    let big = validate_const_materializable_typed_owned_layout(
        &checked.typed,
        "Samples",
        &layout,
        &value,
        ByteOrder::BigEndian,
    )
    .expect("float custody should bind target byte order");
    assert_eq!(
        big.bytes(),
        &[
            0x3f, 0xc0, 0x00, 0x00, 0, 0, 0, 0, 0x7f, 0xf0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ]
    );
    assert_ne!(
        little.non_authoritative_materialization_report_fingerprint(),
        big.non_authoritative_materialization_report_fingerprint()
    );

    let mut nan = value.clone();
    let BuildTimeValue::Struct { fields, .. } = &mut nan else {
        unreachable!("fixture value is a record")
    };
    fields[1].1 = BuildTimeValue::Float(f64::NAN);
    let error = validate_const_materializable_typed_owned_layout(
        &checked.typed,
        "Samples",
        &layout,
        &nan,
        ByteOrder::LittleEndian,
    )
    .expect_err("NaN still requires exact raw-representation evidence");
    assert!(error.0.contains("exact raw-NaN realization"), "{error:?}");

    let mut rounded = value;
    let BuildTimeValue::Struct { fields, .. } = &mut rounded else {
        unreachable!("fixture value is a record")
    };
    fields[0].1 = BuildTimeValue::Float(0.1);
    let error = validate_const_materializable_typed_owned_layout(
        &checked.typed,
        "Samples",
        &layout,
        &rounded,
        ByteOrder::LittleEndian,
    )
    .expect_err("f64 custody cannot be rounded into a purported exact binary32 value");
    assert!(error.0.contains("exact binary32 value"), "{error:?}");
}

#[test]
fn source_machine_owned_fixed_array_materializes_through_the_typed_bridge() {
    let main_path = write_program(
        "source-owned-fixed-array",
        r#"
use omega::language::core::layout;

data ArrayLayout { }
machine ArrayLayout::plan(&mut self, schema: Schema) -> Plan {
    let mut entries: [FieldEntry; 64];
    entries[0] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 4 },
    };
    Plan { entries: entries, entry_count: 1,
           size_fixed: 12, size_is_dynamic: false, align: 2 }
}
data Samples { values: [u16; 3]; }
machine make_samples() -> Samples {
    Samples { values: [258, 772, 1286] }
}
data Main { }
machine Main::main(&mut self) { }
"#,
    );
    let checked = compile_to_checked(CheckedCompileRequest::new(&main_path, None))
        .expect("owned array producer should check");
    let report = compute_layout_plan(&checked.typed, "ArrayLayout::plan", "Samples", None)
        .expect("owned array should have one whole-field placement");
    let mut bytes = [0xa5; 12];
    evaluate_and_materialize_typed_owned_layout_into(
        &checked.typed,
        "make_samples",
        "Samples",
        &report,
        ByteOrder::LittleEndian,
        &mut bytes,
    )
    .expect("source-owned fixed array should evaluate and materialize atomically");
    assert_eq!(&bytes[4..10], &[2, 1, 4, 3, 6, 5]);
    assert!(bytes[..4].iter().chain(&bytes[10..]).all(|byte| *byte == 0));
}

#[test]
fn source_machine_owned_fixed_array_materializes_through_element_at_tiling() {
    let main_path = write_program(
        "source-owned-fixed-array-element-tiling",
        r#"
use omega::language::core::layout;

data ArrayLayout { }
machine ArrayLayout::plan(&mut self, schema: Schema) -> Plan {
    let mut entries: [FieldEntry; 64];
    entries[0] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 12 },
    };
    entries[1] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 4 },
    };
    entries[2] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 8 },
    };
    Plan { entries: entries, entry_count: 3,
           size_fixed: 16, size_is_dynamic: false, align: 2 }
}
data Samples { values: [u16; 3]; }
machine make_samples() -> Samples {
    Samples { values: [258, 772, 1286] }
}
data Main { }
machine Main::main(&mut self) { }
"#,
    );
    let checked = compile_to_checked(CheckedCompileRequest::new(&main_path, None))
        .expect("tiled fixed array should check");
    let report = compute_layout_plan(&checked.typed, "ArrayLayout::plan", "Samples", None)
        .expect("one element At per fixed-array element should validate");
    assert_eq!(report.offsets, None);
    assert_eq!(
        report
            .entries
            .iter()
            .map(|entry| match entry.placement {
                LayoutPlacementReport::At { offset } => offset,
                _ => panic!("fixed-array tiling should retain only At entries"),
            })
            .collect::<Vec<_>>(),
        vec![4, 8, 12]
    );

    let mut little = [0xa5; 16];
    evaluate_and_materialize_typed_owned_layout_into(
        &checked.typed,
        "make_samples",
        "Samples",
        &report,
        ByteOrder::LittleEndian,
        &mut little,
    )
    .expect("fixed-array elements should materialize at their canonical destinations");
    assert_eq!(&little[4..14], &[2, 1, 0, 0, 4, 3, 0, 0, 6, 5]);
    assert!(
        little[..4]
            .iter()
            .chain(&little[14..])
            .all(|byte| *byte == 0)
    );

    let mut big = [0xa5; 16];
    evaluate_and_materialize_typed_owned_layout_into(
        &checked.typed,
        "make_samples",
        "Samples",
        &report,
        ByteOrder::BigEndian,
        &mut big,
    )
    .expect("tiling should preserve Omega-selected target byte order");
    assert_eq!(&big[4..14], &[1, 2, 0, 0, 3, 4, 0, 0, 5, 6]);
}

#[test]
fn source_machine_owned_fixed_record_array_materializes_through_the_typed_bridge() {
    let main_path = write_program(
        "source-owned-fixed-record-array",
        r#"
use omega::language::core::layout;

data ArrayLayout { }
machine ArrayLayout::plan(&mut self, schema: Schema) -> Plan {
    let mut entries: [FieldEntry; 64];
    entries[0] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 8 },
    };
    Plan { entries: entries, entry_count: 1,
           size_fixed: 32, size_is_dynamic: false, align: 4 }
}
data Pair { low: u16; high: u32; }
data Samples { pairs: [Pair; 2]; }
machine make_samples() -> Samples {
    let pairs: [Pair; 2];
    pairs[0] = Pair { low: 258, high: 100992003 };
    pairs[1] = Pair { low: 2055, high: 202050057 };
    Samples { pairs: pairs }
}
data Main { }
machine Main::main(&mut self) { }
"#,
    );
    let checked = compile_to_checked(CheckedCompileRequest::new(&main_path, None))
        .expect("owned fixed-record array should check");
    let report = compute_layout_plan(&checked.typed, "ArrayLayout::plan", "Samples", None)
        .expect("fixed-record array should have one whole-field placement");
    let mut bytes = [0xa5; 32];
    evaluate_and_materialize_typed_owned_layout_into(
        &checked.typed,
        "make_samples",
        "Samples",
        &report,
        ByteOrder::LittleEndian,
        &mut bytes,
    )
    .expect("source-owned fixed-record array should materialize atomically");
    assert_eq!(
        &bytes[8..24],
        &[2, 1, 0, 0, 3, 4, 5, 6, 7, 8, 0, 0, 9, 10, 11, 12]
    );
    assert!(bytes[..8].iter().chain(&bytes[24..]).all(|byte| *byte == 0));

    let mut big_endian = [0xa5; 32];
    evaluate_and_materialize_typed_owned_layout_into(
        &checked.typed,
        "make_samples",
        "Samples",
        &report,
        ByteOrder::BigEndian,
        &mut big_endian,
    )
    .expect("Omega target byte order should compose through fixed-record arrays");
    assert_eq!(
        &big_endian[8..24],
        &[1, 2, 0, 0, 6, 5, 4, 3, 8, 7, 0, 0, 12, 11, 10, 9]
    );
    assert!(
        big_endian[..8]
            .iter()
            .chain(&big_endian[24..])
            .all(|byte| *byte == 0)
    );

    let malformed = BuildTimeValue::Struct {
        type_name: "Samples".to_owned(),
        fields: vec![(
            "pairs".to_owned(),
            BuildTimeValue::Array(vec![BuildTimeValue::Struct {
                type_name: "Pair".to_owned(),
                fields: vec![
                    ("low".to_owned(), BuildTimeValue::Int(258)),
                    ("high".to_owned(), BuildTimeValue::Int(100992003)),
                ],
            }]),
        )],
    };
    let mut unchanged = [0x5a; 32];
    let error = materialize_typed_owned_layout_into(
        &checked.typed,
        "Samples",
        &report,
        &malformed,
        ByteOrder::LittleEndian,
        &mut unchanged,
    )
    .expect_err("a short record array must reject before destination mutation");
    assert!(error.0.contains("has 1 elements, expected 2"));
    assert_eq!(unchanged, [0x5a; 32]);

    let wrong_element = BuildTimeValue::Struct {
        type_name: "Samples".to_owned(),
        fields: vec![(
            "pairs".to_owned(),
            BuildTimeValue::Array(vec![
                BuildTimeValue::Struct {
                    type_name: "Samples".to_owned(),
                    fields: vec![
                        ("low".to_owned(), BuildTimeValue::Int(258)),
                        ("high".to_owned(), BuildTimeValue::Int(100992003)),
                    ],
                },
                BuildTimeValue::Struct {
                    type_name: "Pair".to_owned(),
                    fields: vec![
                        ("low".to_owned(), BuildTimeValue::Int(2055)),
                        ("high".to_owned(), BuildTimeValue::Int(202050057)),
                    ],
                },
            ]),
        )],
    };
    let mut unchanged = [0x5a; 32];
    let error = materialize_typed_owned_layout_into(
        &checked.typed,
        "Samples",
        &report,
        &wrong_element,
        ByteOrder::LittleEndian,
        &mut unchanged,
    )
    .expect_err("a wrong nested record identity must reject before mutation");
    assert!(error.0.contains("does not match `Pair`"));
    assert_eq!(unchanged, [0x5a; 32]);

    let late_invalid_scalar = BuildTimeValue::Struct {
        type_name: "Samples".to_owned(),
        fields: vec![(
            "pairs".to_owned(),
            BuildTimeValue::Array(vec![
                BuildTimeValue::Struct {
                    type_name: "Pair".to_owned(),
                    fields: vec![
                        ("low".to_owned(), BuildTimeValue::Int(258)),
                        ("high".to_owned(), BuildTimeValue::Int(100992003)),
                    ],
                },
                BuildTimeValue::Struct {
                    type_name: "Pair".to_owned(),
                    fields: vec![
                        ("low".to_owned(), BuildTimeValue::Int(-1)),
                        ("high".to_owned(), BuildTimeValue::Int(202050057)),
                    ],
                },
            ]),
        )],
    };
    let mut unchanged = [0x5a; 32];
    let error = materialize_typed_owned_layout_into(
        &checked.typed,
        "Samples",
        &report,
        &late_invalid_scalar,
        ByteOrder::LittleEndian,
        &mut unchanged,
    )
    .expect_err("a late invalid nested scalar must reject before destination mutation");
    assert!(error.0.contains("outside `u16`"));
    assert_eq!(unchanged, [0x5a; 32]);
}

#[test]
fn source_machine_owned_closed_generic_records_use_exact_specialized_shapes() {
    let main_path = write_program(
        "source-owned-closed-generic-records",
        r#"
use omega::language::core::layout;

data Split { }
machine Split::plan(&mut self, schema: Schema) -> Plan {
    let mut entries: [FieldEntry; 64];
    entries[0] = FieldEntry {
        key: schema.fields[1].key,
        placement: FieldPlan::At { offset: 16 },
    };
    entries[1] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 2 },
    };
    entries[2] = FieldEntry {
        key: schema.fields[1].key,
        placement: FieldPlan::At { offset: 8 },
    };
    Plan { entries: entries, entry_count: 3,
           size_fixed: 24, size_is_dynamic: false, align: 4 }
}
data Evidence { case Only; }
data Cell<T> { proof [erased]: Evidence; value: T; }
data Samples { narrow: Cell<u16>; wide: [Cell<u32>; 2]; }
machine make_samples() -> Samples {
    let narrow: Cell<u16> = Cell { proof: Evidence::Only, value: 258 };
    let wide0: Cell<u32> = Cell { proof: Evidence::Only, value: 50595078 };
    let wide1: Cell<u32> = Cell { proof: Evidence::Only, value: 117967114 };
    let wide: [Cell<u32>; 2] = [wide0, wide1];
    Samples { narrow: narrow, wide: wide }
}
data Main { }
machine Main::main(&mut self) { }
"#,
    );
    let checked = compile_to_checked(CheckedCompileRequest::new(&main_path, None))
        .expect("closed generic record specializations should check");
    let narrow = checked
        .typed
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Cell<u16>")
        .expect("the narrow closed specialization should be synthesized");
    let wide = checked
        .typed
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Cell<u32>")
        .expect("the wide closed specialization should be synthesized");
    assert_ne!(narrow.symbol, wide.symbol);
    assert!(narrow.type_parameters.is_empty() && wide.type_parameters.is_empty());

    let report = compute_layout_plan(&checked.typed, "Split::plan", "Samples", None)
        .expect("closed specialized records should derive exact nested extents");
    let mut little = [0xa5; 24];
    evaluate_and_materialize_typed_owned_layout_into(
        &checked.typed,
        "make_samples",
        "Samples",
        &report,
        ByteOrder::LittleEndian,
        &mut little,
    )
    .expect("distinct closed specializations should materialize through their concrete shapes");
    assert_eq!(&little[2..4], &[2, 1]);
    assert_eq!(&little[8..12], &[6, 5, 4, 3]);
    assert_eq!(&little[16..20], &[10, 9, 8, 7]);
    assert!(
        little[..2]
            .iter()
            .chain(&little[4..8])
            .chain(&little[12..16])
            .chain(&little[20..])
            .all(|byte| *byte == 0)
    );

    let mut big = [0xa5; 24];
    evaluate_and_materialize_typed_owned_layout_into(
        &checked.typed,
        "make_samples",
        "Samples",
        &report,
        ByteOrder::BigEndian,
        &mut big,
    )
    .expect("specialized records should retain the selected target byte order");
    assert_eq!(&big[2..4], &[1, 2]);
    assert_eq!(&big[8..12], &[3, 4, 5, 6]);
    assert_eq!(&big[16..20], &[7, 8, 9, 10]);

    fn cell(type_name: &str, proof_name: &str, value: i64) -> BuildTimeValue {
        BuildTimeValue::Struct {
            type_name: type_name.to_owned(),
            fields: vec![
                (
                    proof_name.to_owned(),
                    BuildTimeValue::Case {
                        variant: "Only".to_owned(),
                        payload: Vec::new(),
                    },
                ),
                ("value".to_owned(), BuildTimeValue::Int(value)),
            ],
        }
    }
    fn samples(narrow_type: &str, narrow_proof: &str, wide_values: &[i64]) -> BuildTimeValue {
        BuildTimeValue::Struct {
            type_name: "Samples".to_owned(),
            fields: vec![
                ("narrow".to_owned(), cell(narrow_type, narrow_proof, 258)),
                (
                    "wide".to_owned(),
                    BuildTimeValue::Array(
                        wide_values
                            .iter()
                            .map(|value| cell("Cell<u32>", "proof", *value))
                            .collect(),
                    ),
                ),
            ],
        }
    }
    for (description, malformed, expected) in [
        (
            "wrong specialization",
            samples("Cell<u32>", "proof", &[50_595_078, 117_967_114]),
            "does not match `Cell<u16>`",
        ),
        (
            "missing erased semantic field",
            samples("Cell<u16>", "forged", &[50_595_078, 117_967_114]),
            "no field `proof`",
        ),
        (
            "wrong specialized array count",
            samples("Cell<u16>", "proof", &[50_595_078]),
            "has 1 elements, expected 2",
        ),
        (
            "late out-of-range specialized scalar",
            samples("Cell<u16>", "proof", &[50_595_078, -1]),
            "outside `u32`",
        ),
    ] {
        let mut unchanged = [0x5a; 24];
        let error = materialize_typed_owned_layout_into(
            &checked.typed,
            "Samples",
            &report,
            &malformed,
            ByteOrder::LittleEndian,
            &mut unchanged,
        )
        .expect_err(description);
        assert!(error.0.contains(expected), "{description}: {error:?}");
        assert_eq!(unchanged, [0x5a; 24], "{description}");
    }
}

#[test]
fn source_machine_owned_erased_fields_are_semantic_but_not_materialized() {
    let main_path = write_program(
        "source-owned-erased-field",
        r#"
use omega::language::core::layout;

data Spread { }
machine Spread::plan(&mut self, schema: Schema) -> Plan {
    let mut entries: [FieldEntry; 64];
    entries[0] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 4 },
    };
    entries[1] = FieldEntry {
        key: schema.fields[1].key,
        placement: FieldPlan::At { offset: 12 },
    };
    Plan { entries: entries, entry_count: 2,
           size_fixed: 20, size_is_dynamic: false, align: 4 }
}
data Evidence { case Only; }
data Certified {
    left: u16;
    proof [erased]: Evidence;
    right: u32;
}
machine make_certified() -> Certified {
    Certified { left: 258, proof: Evidence::Only, right: 100992003 }
}
data Main { }
machine Main::main(&mut self) { }
"#,
    );
    let checked = compile_to_checked(CheckedCompileRequest::new(&main_path, None))
        .expect("erased field producer should check");
    let report = compute_layout_plan(&checked.typed, "Spread::plan", "Certified", None)
        .expect("only relevant fields should enter the normalized layout");
    let mut bytes = [0xa5; 20];
    evaluate_and_materialize_typed_owned_layout_into(
        &checked.typed,
        "make_certified",
        "Certified",
        &report,
        ByteOrder::LittleEndian,
        &mut bytes,
    )
    .expect("erased semantic evidence should contribute no physical bytes");
    assert_eq!(&bytes[4..6], &[2, 1]);
    assert_eq!(&bytes[12..16], &[3, 4, 5, 6]);
    assert!(
        bytes[..4]
            .iter()
            .chain(&bytes[6..12])
            .chain(&bytes[16..])
            .all(|byte| *byte == 0)
    );
}

#[test]
fn source_machine_owned_nested_erased_fields_are_exact_and_storage_free() {
    let main_path = write_program(
        "source-owned-nested-erased-field",
        r#"
use omega::language::core::layout;

data Whole { }
machine Whole::plan(&mut self, schema: Schema) -> Plan {
    let mut entries: [FieldEntry; 64];
    entries[0] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 4 },
    };
    Plan { entries: entries, entry_count: 1,
           size_fixed: 16, size_is_dynamic: false, align: 4 }
}
data Evidence { case Only; }
data Certified {
    left: u16;
    proof [erased]: Evidence;
    right: u32;
}
data Envelope { certified: Certified; }
machine make_envelope() -> Envelope {
    Envelope {
        certified: Certified {
            left: 258,
            proof: Evidence::Only,
            right: 100992003,
        },
    }
}
data Main { }
machine Main::main(&mut self) { }
"#,
    );
    let checked = compile_to_checked(CheckedCompileRequest::new(&main_path, None))
        .expect("nested erased field producer should check");
    let report = compute_layout_plan(&checked.typed, "Whole::plan", "Envelope", None)
        .expect("the nested record should retain one relevant whole-field extent");
    let mut bytes = [0xa5; 16];
    evaluate_and_materialize_typed_owned_layout_into(
        &checked.typed,
        "make_envelope",
        "Envelope",
        &report,
        ByteOrder::LittleEndian,
        &mut bytes,
    )
    .expect("nested erased evidence should remain semantic and storage-free");
    assert_eq!(&bytes[4..12], &[2, 1, 0, 0, 3, 4, 5, 6]);
    assert!(bytes[..4].iter().chain(&bytes[12..]).all(|byte| *byte == 0));

    let missing_erased = BuildTimeValue::Struct {
        type_name: "Envelope".to_owned(),
        fields: vec![(
            "certified".to_owned(),
            BuildTimeValue::Struct {
                type_name: "Certified".to_owned(),
                fields: vec![
                    ("left".to_owned(), BuildTimeValue::Int(258)),
                    ("forged".to_owned(), BuildTimeValue::Int(0)),
                    ("right".to_owned(), BuildTimeValue::Int(100992003)),
                ],
            },
        )],
    };
    let mut unchanged = [0x5a; 16];
    let error = materialize_typed_owned_layout_into(
        &checked.typed,
        "Envelope",
        &report,
        &missing_erased,
        ByteOrder::LittleEndian,
        &mut unchanged,
    )
    .expect_err("an unknown same-count field cannot replace erased evidence");
    assert!(error.0.contains("no field `proof`"));
    assert_eq!(unchanged, [0x5a; 16]);
}

#[test]
fn source_machine_owned_record_arrays_omit_each_erased_field() {
    let main_path = write_program(
        "source-owned-record-array-erased-fields",
        r#"
use omega::language::core::layout;

data Whole { }
machine Whole::plan(&mut self, schema: Schema) -> Plan {
    let mut entries: [FieldEntry; 64];
    entries[0] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 4 },
    };
    Plan { entries: entries, entry_count: 1,
           size_fixed: 24, size_is_dynamic: false, align: 4 }
}
data Evidence { case Only; }
data Certified {
    left: u16;
    proof [erased]: Evidence;
    right: u32;
}
data Batch { items: [Certified; 2]; }
machine make_batch() -> Batch {
    let items: [Certified; 2];
    items[0] = Certified {
        left: 258,
        proof: Evidence::Only,
        right: 100992003,
    };
    items[1] = Certified {
        left: 2055,
        proof: Evidence::Only,
        right: 202050057,
    };
    Batch { items: items }
}
data Main { }
machine Main::main(&mut self) { }
"#,
    );
    let checked = compile_to_checked(CheckedCompileRequest::new(&main_path, None))
        .expect("record array with erased fields should check");
    let report = compute_layout_plan(&checked.typed, "Whole::plan", "Batch", None)
        .expect("record array should retain one whole repeated extent");
    let mut bytes = [0xa5; 24];
    evaluate_and_materialize_typed_owned_layout_into(
        &checked.typed,
        "make_batch",
        "Batch",
        &report,
        ByteOrder::LittleEndian,
        &mut bytes,
    )
    .expect("every erased array-element field should contribute no bytes");
    assert_eq!(
        &bytes[4..20],
        &[2, 1, 0, 0, 3, 4, 5, 6, 7, 8, 0, 0, 9, 10, 11, 12]
    );
    assert!(bytes[..4].iter().chain(&bytes[20..]).all(|byte| *byte == 0));

    let mut big_endian = [0xa5; 24];
    evaluate_and_materialize_typed_owned_layout_into(
        &checked.typed,
        "make_batch",
        "Batch",
        &report,
        ByteOrder::BigEndian,
        &mut big_endian,
    )
    .expect("byte order should compose independently of erased element fields");
    assert_eq!(
        &big_endian[4..20],
        &[1, 2, 0, 0, 6, 5, 4, 3, 8, 7, 0, 0, 12, 11, 10, 9]
    );
    assert!(
        big_endian[..4]
            .iter()
            .chain(&big_endian[20..])
            .all(|byte| *byte == 0)
    );
}

#[test]
fn tiled_record_arrays_keep_erased_fields_semantic_and_storage_free() {
    let main_path = write_program(
        "source-owned-tiled-record-array-erased-fields",
        r#"
use omega::language::core::layout;

data Tiled { }
machine Tiled::plan(&mut self, schema: Schema) -> Plan {
    let mut entries: [FieldEntry; 64];
    entries[0] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 16 },
    };
    entries[1] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 4 },
    };
    Plan { entries: entries, entry_count: 2,
           size_fixed: 28, size_is_dynamic: false, align: 4 }
}
data Evidence { case Only; }
data Certified {
    left: u16;
    proof [erased]: Evidence;
    right: u32;
}
data Batch { items: [Certified; 2]; }
machine make_batch() -> Batch {
    let items: [Certified; 2];
    items[0] = Certified {
        left: 258,
        proof: Evidence::Only,
        right: 100992003,
    };
    items[1] = Certified {
        left: 2055,
        proof: Evidence::Only,
        right: 202050057,
    };
    Batch { items: items }
}
data Main { }
machine Main::main(&mut self) { }
"#,
    );
    let checked = compile_to_checked(CheckedCompileRequest::new(&main_path, None))
        .expect("tiled record array with erased fields should check");
    let report = compute_layout_plan(&checked.typed, "Tiled::plan", "Batch", None)
        .expect("the repeated field should accept one At per physical element");
    let mut bytes = [0xa5; 28];
    evaluate_and_materialize_typed_owned_layout_into(
        &checked.typed,
        "make_batch",
        "Batch",
        &report,
        ByteOrder::LittleEndian,
        &mut bytes,
    )
    .expect("erased fields should stay semantic while each element tiles physically");
    assert_eq!(&bytes[4..12], &[2, 1, 0, 0, 3, 4, 5, 6]);
    assert_eq!(&bytes[16..24], &[7, 8, 0, 0, 9, 10, 11, 12]);
    assert!(
        bytes[..4]
            .iter()
            .chain(&bytes[12..16])
            .chain(&bytes[24..])
            .all(|byte| *byte == 0)
    );
}

#[test]
fn source_machine_owned_all_erased_record_materializes_only_plan_storage() {
    let main_path = write_program(
        "source-owned-all-erased-record",
        r#"
use omega::language::core::layout;

data Whole { }
machine Whole::plan(&mut self, schema: Schema) -> Plan {
    let entries: [FieldEntry; 64];
    Plan { entries: entries, entry_count: 0,
           size_fixed: 8, size_is_dynamic: false, align: 4 }
}
data Evidence { case Only; }
data ProofBox { proof [erased]: Evidence; }
machine make_proof_box() -> ProofBox {
    ProofBox { proof: Evidence::Only }
}
data Main { }
machine Main::main(&mut self) { }
"#,
    );
    let checked = compile_to_checked(CheckedCompileRequest::new(&main_path, None))
        .expect("all-erased owned record should remain a checked semantic value");
    let report = compute_layout_plan(&checked.typed, "Whole::plan", "ProofBox", None)
        .expect("an all-erased owned record should require no physical field entries");
    assert!(report.entries.is_empty());
    let mut bytes = [0xa5; 8];
    evaluate_and_materialize_typed_owned_layout_into(
        &checked.typed,
        "make_proof_box",
        "ProofBox",
        &report,
        ByteOrder::LittleEndian,
        &mut bytes,
    )
    .expect("all-erased semantic content should contribute no bytes");
    assert_eq!(bytes, [0; 8]);

    let missing_proof = BuildTimeValue::Struct {
        type_name: "ProofBox".to_owned(),
        fields: Vec::new(),
    };
    let mut unchanged = [0x5a; 8];
    let error = materialize_typed_owned_layout_into(
        &checked.typed,
        "ProofBox",
        &report,
        &missing_proof,
        ByteOrder::LittleEndian,
        &mut unchanged,
    )
    .expect_err("erased storage omission must not make the semantic term optional");
    assert!(error.0.contains("0 fields, expected 1"));
    assert_eq!(unchanged, [0x5a; 8]);
}
