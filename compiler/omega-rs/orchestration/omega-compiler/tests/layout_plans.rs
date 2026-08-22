//! L3 of the LAYOUTS ladder: the zero-codegen plan pipeline, end to end. The
//! pilot policy is the brief's headline claim made real -- THE C ABI AS ~15
//! LINES OF OMEGA: an effect-free `CLayout::plan` machine (round up to the
//! field's alignment, place, track the widest alignment, round the total) is
//! evaluated at BUILD TIME against a compiler-materialized Schema, and the
//! compiler VALIDATES the plan before reporting it. A buggy policy is a
//! compile error, never unsafety -- which is also why the policy's scratch
//! arithmetic may honestly declare Wrapping: plan validation owns soundness.

use omega_compiler::{
    AggregateFieldSchema, AggregateFieldValue, BuildTimeValue, ByteOrder, ConsumptionInstant,
    EntryStubId, IntegerInterpretation, LayoutPlacementReport, MaterializationAction,
    MaterializationContext, RelocationTarget, ScalarFieldSchema, ScalarFieldValue,
    SymbolicFieldValue, compile_to_checked, compute_layout_plan, decode_scalar_layout,
    derive_symbolic_materialization, evaluate_and_materialize_typed_owned_layout_into,
    materialize_aggregate_layout_into, materialize_scalar_layout_into,
    materialize_typed_owned_layout_into,
};
use omega_layout::{DataShape, build_layout_plan};
use omega_target::NativeTarget;
use std::fs;
use std::path::{Path, PathBuf};

fn write_program(name: &str, source: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("omega-layout-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create temp program dir");
    let main_path = dir.join("main.omg");
    fs::write(&main_path, source).expect("write layout program");
    main_path
}

/// The vocabulary (mirrors omega/language/core/layout.omg) + the CLayout
/// policy + a UEFI-ish schema. Inlined because temp-dir programs cannot
/// resolve `use omega::...` library paths.
const PILOT: &str = r#"
data FieldKind { case Scalar; case Text; case Nested; case Repeated; }
data SchemaField {
    key: u64;
    size: u64 [0..=4096];
    align: u64 [1..=16];
    number: i64;
    kind: FieldKind;
}
data Schema {
    fields: [SchemaField; 32];
    field_count: u64 [0..=32];
}
data FieldPlan {
    case At(offset: u64);
    case Bits(container: u64, container_width: u64, destination_lsb: u64, source_lsb: u64, width: u64);
    case Varint(tag: u64);
    case LengthPrefixed(tag: u64);
}
data FieldEntry { key: u64; placement: FieldPlan; }
data Plan {
    entries: [FieldEntry; 64];
    entry_count: u64;
    size_fixed: u64;
    size_is_dynamic: bool;
    align: u64;
}

// The C layout rule as a build-time Omega machine. Scratch accumulators are
// DECLARED Wrapping (policy-internal arithmetic; the compiler's plan
// validation catches any garbage plan), and the padding uses the modulo form
// ((a - offset % a) % a) so no division is needed.
data CLayout {
    entries: [FieldEntry; 64];
    index: u64 in Wrapping;
    offset: u64 in Wrapping;
    widest: u64 in Wrapping;
    fsize: u64 in Wrapping;
    falign: u64 in Wrapping;
    pad: u64 in Wrapping;
}
machine CLayout::plan(&mut self, schema: Schema) -> Plan {
    self.evaluate(schema, 256)
}

machine CLayout::evaluate(&mut self, schema: Schema, fuel: u64 [1..=256]) -> Plan
terminates by fuel;
{
    self.index = 0;
    self.offset = 0;
    self.widest = 1;
    transition { _ -> place_loop(schema, fuel) }

    state place_loop(&mut self, schema: Schema, fuel: u64 [1..=256]) {
        transition fuel > 1 && self.index < 32 && self.index < schema.field_count {
            true -> read_field(schema, fuel - 1)
            _ -> done(schema)
        }
    }
    state read_field(&mut self, schema: Schema, fuel: u64 [1..=256]) {
        self.fsize = schema.fields[self.index].size;
        self.falign = schema.fields[self.index].align;
        // C rule: round up to the field's alignment, place, advance.
        self.pad = (self.falign - self.offset % self.falign) % self.falign;
        self.offset = self.offset + self.pad;
        transition fuel > 1 && self.index < 32 {
            true -> place_field(schema, fuel - 1)
            _ -> done(schema)
        }
    }
    state place_field(&mut self, schema: Schema, fuel: u64 [1..=256]) {
        self.entries[self.index] = FieldEntry {
            key: schema.fields[self.index].key,
            placement: FieldPlan::At { offset: self.offset as u64 },
        };
        self.offset = self.offset + self.fsize;
        transition fuel > 1 {
            true -> choose_widen(schema, fuel - 1)
            _ -> done(schema)
        }
    }
    state choose_widen(&mut self, schema: Schema, fuel: u64 [1..=256]) {
        transition {
            fuel > 1 && self.widest < self.falign -> widen(schema, fuel - 1)
            fuel > 1 -> advance(schema, fuel - 1)
            _ -> done(schema)
        }
    }
    state widen(&mut self, schema: Schema, fuel: u64 [1..=256]) {
        self.widest = self.falign;
        transition fuel > 1 {
            true -> advance(schema, fuel - 1)
            _ -> done(schema)
        }
    }
    state advance(&mut self, schema: Schema, fuel: u64 [1..=256]) {
        self.index = self.index + 1;
        transition fuel > 1 {
            true -> place_loop(schema, fuel - 1)
            _ -> done(schema)
        }
    }
    state done(&mut self, schema: Schema) -> Plan {
        // Round the total size up to the struct alignment (the C tail rule).
        self.pad = (self.widest - self.offset % self.widest) % self.widest;
        Plan {
            entries: self.entries,
            entry_count: schema.field_count,
            size_fixed: (self.offset + self.pad) as u64,
            size_is_dynamic: false,
            align: self.widest as u64,
        }
    }
}

data GdtEntryish {
    limit_low: u16;
    base_low: u32;
    flags: u8;
    base_high: u64;
}

data Main { }
machine Main::main(&mut self) { }
"#;

/// L4: plan-laid VALUE TYPES. The run canary's `gdt: Spread16<Gdtish>` field
/// resolves to a synthesized record whose NATIVE placement is the validated
/// plan's -- asserted here directly against the layout plan the backend
/// consumes, because placement is deliberately unobservable from inside the
/// language (the run canary proves no-miscompile; this proves the override
/// actually fired: native packing would give offsets [0,4,8,16], size 24).
#[test]
fn plan_laid_value_types_are_placed_by_their_plan() {
    let canary = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("compiler crate should live under compiler/orchestration/omega-compiler")
        .join("canaries/pass/layouts/runtime_plan_laid_value_field_exit/main.omg");
    let mut checked = compile_to_checked(&canary, None).expect("plan-laid canary should compile");

    // The pipeline recorded the validated plan on the typed trees.
    assert_eq!(checked.typed.plan_laid_layouts.len(), 1);
    let recorded = &checked.typed.plan_laid_layouts[0];
    assert_eq!(recorded.data_name, "Spread16<Gdtish>");
    let plan_laid_data_symbol = recorded.data_symbol;
    let schema_symbol = recorded.schema_symbol;
    let schema_field_symbols = recorded.schema_field_symbols.clone();
    let policy_symbol = recorded.policy_symbol;
    let policy_plan_machine_symbol = recorded.policy_plan_machine_symbol;
    let plan_laid_data = checked
        .typed
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == plan_laid_data_symbol)
        .expect("exact synthesized plan-laid data");
    assert_eq!(plan_laid_data.name.as_str(), recorded.data_name);
    assert_eq!(recorded.offsets, vec![0, 16, 32, 48]);
    assert_eq!(recorded.size, 64);
    assert_eq!(recorded.align, 16);

    // And the backend layout plan bakes those offsets into the synthesized
    // record's FieldLayouts.
    let target = NativeTarget::from_omega_target_name(None).expect("host target");
    let layouts = build_layout_plan(&checked, target).expect("layout plan should build");
    let data_layout = layouts
        .data_layouts
        .iter()
        .map(|(_, layout)| layout)
        .find(|layout| layout.name.as_str() == "Spread16<Gdtish>")
        .expect("the synthesized plan-laid record should be laid out");
    assert_eq!(data_layout.layout.size, 64);
    assert_eq!(data_layout.layout.alignment, 16);
    let DataShape::Record { fields } = &data_layout.shape else {
        panic!("plan-laid data should be a record");
    };
    let offsets: Vec<usize> = layouts
        .fields
        .span_or_empty(*fields)
        .iter()
        .map(|field| field.offset)
        .collect();
    assert_eq!(offsets, vec![0, 16, 32, 48]);

    checked.typed.plan_laid_layouts[0].data_name = "diagnostic-only-layout-name".to_owned();
    psi_validation::validate_program(&checked.typed)
        .expect("presentation-name drift must not change retained plan identity");
    let layouts = build_layout_plan(&checked, target)
        .expect("presentation-name drift must not redirect a plan-laid layout");
    let data_layout = layouts
        .data_layouts
        .iter()
        .map(|(_, layout)| layout)
        .find(|layout| layout.symbol == plan_laid_data_symbol)
        .expect("exact synthesized plan-laid record should remain selected by symbol");
    assert_eq!(data_layout.layout.size, 64);

    let (main_symbol, main_field_symbols) = {
        let main = checked
            .typed
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == "Main")
            .expect("fixture Main data");
        let symbols = checked
            .typed
            .data_members(main)
            .iter()
            .filter_map(|member| match member {
                psi_typed_trees::data::DataMember::Field(field) => Some(field.symbol),
                psi_typed_trees::data::DataMember::Variant(_) => None,
            })
            .collect::<Vec<_>>();
        (main.symbol, symbols)
    };

    checked.typed.plan_laid_layouts[0].schema_symbol = main_symbol;
    let diagnostics = psi_validation::validate_program(&checked.typed)
        .expect_err("substituted source schema identity must fail closed");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("changed its exact source schema field identity inventory")
    }));
    checked.typed.plan_laid_layouts[0].schema_symbol = schema_symbol;

    checked.typed.plan_laid_layouts[0].schema_symbol = main_symbol;
    checked.typed.plan_laid_layouts[0].schema_field_symbols = main_field_symbols;
    let diagnostics = psi_validation::validate_program(&checked.typed)
        .expect_err("coordinated source schema substitution must fail closed");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("changed its exact schema-to-synthesized field correspondence")
    }));
    checked.typed.plan_laid_layouts[0].schema_symbol = schema_symbol;
    checked.typed.plan_laid_layouts[0].schema_field_symbols = schema_field_symbols;

    let first_offset = checked.typed.plan_laid_layouts[0].offsets[0];
    checked.typed.plan_laid_layouts[0].offsets[0] = first_offset + 1;
    let diagnostics = psi_validation::validate_program(&checked.typed)
        .expect_err("drifted flattened geometry must fail closed");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("changed its exact validated geometry projection")
    }));
    checked.typed.plan_laid_layouts[0].offsets[0] = first_offset;

    checked.typed.plan_laid_layouts[0].policy_symbol = main_symbol;
    let diagnostics = psi_validation::validate_program(&checked.typed)
        .expect_err("substituted nominal policy identity must fail closed");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("changed its exact nominal policy binding")
    }));
    checked.typed.plan_laid_layouts[0].policy_symbol = policy_symbol;

    checked.typed.plan_laid_layouts[0].policy_plan_machine_symbol = main_symbol;
    let diagnostics = psi_validation::validate_program(&checked.typed)
        .expect_err("substituted policy plan machine must fail closed");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("no longer names its exact policy plan machine")
    }));
    checked.typed.plan_laid_layouts[0].policy_plan_machine_symbol = policy_plan_machine_symbol;

    checked.typed.plan_laid_layouts[0].data_symbol = main_symbol;
    let diagnostics = psi_validation::validate_program(&checked.typed)
        .expect_err("substituted plan-laid data identity must fail validation");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("changed its exact synthesized field identity inventory")
    }));
    let diagnostic = build_layout_plan(&checked, target)
        .expect_err("substituted plan-laid data identity must fail closed");
    assert!(
        diagnostic
            .message
            .contains("plan-laid data `Main` changed its exact field identity inventory")
    );
}

#[test]
fn plan_laid_compact_bits_retain_validated_fragment_geometry() {
    let canary = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("compiler crate should live under compiler/orchestration/omega-compiler")
        .join("canaries/pass/layouts/runtime_plan_laid_compact_bits_exit/main.omg");
    let checked = compile_to_checked(&canary, None).expect("compact-bit canary should compile");

    assert_eq!(checked.typed.plan_laid_layouts.len(), 1);
    let recorded = &checked.typed.plan_laid_layouts[0];
    assert_eq!(recorded.data_name, "CompactBits<PackedFlags>");
    assert_eq!(recorded.offsets, vec![0, 0, 0]);
    assert_eq!(recorded.size, 1);
    assert_eq!(recorded.align, 1);
    assert_eq!(recorded.bit_fields.len(), 3);
    assert_eq!(recorded.bit_fields[0].fragments.len(), 1);
    assert_eq!(recorded.bit_fields[1].fragments.len(), 1);
    assert_eq!(recorded.bit_fields[2].fragments.len(), 2);
    assert_eq!(recorded.bit_fields[2].fragments[0].source_lsb, 0);
    assert_eq!(recorded.bit_fields[2].fragments[1].source_lsb, 2);

    let target = NativeTarget::from_omega_target_name(None).expect("host target");
    let layouts = build_layout_plan(&checked, target).expect("layout plan should build");
    let data_layout = layouts
        .data_layouts
        .iter()
        .map(|(_, layout)| layout)
        .find(|layout| layout.name.as_str() == "CompactBits<PackedFlags>")
        .expect("the synthesized compact-bit record should be laid out");
    let DataShape::Record { fields } = &data_layout.shape else {
        panic!("compact-bit data should be a record");
    };
    let fields = layouts.fields.span_or_empty(*fields);
    assert_eq!(fields.len(), 3);
    assert_eq!(
        fields.iter().map(|field| field.offset).collect::<Vec<_>>(),
        vec![0, 0, 0]
    );
    assert_eq!(
        layouts
            .bit_field(fields[2].symbol)
            .expect("split field should retain bit geometry")
            .fragments
            .len(),
        2
    );
}

#[test]
fn c_layout_policy_plans_a_uefi_ish_schema() {
    let main_path = write_program("clayout-pilot", PILOT);
    let checked = compile_to_checked(&main_path, None).expect("pilot should compile");

    let report = compute_layout_plan(&checked.typed, "CLayout::plan", "GdtEntryish")
        .expect("the C layout plan should evaluate and validate");

    // u16 @ 0, u32 @ 4 (padded past 2), u8 @ 8, u64 @ 16 (padded past 9);
    // size 24 (rounded to align 8).
    assert_eq!(report.offsets, Some(vec![0, 4, 8, 16]));
    assert_eq!(report.size, Some(24));
    assert_eq!(report.align, 8);
}

#[test]
fn fixed_primitive_arrays_are_reflected_as_one_repeated_at_field() {
    let main_path = write_program(
        "fixed-array-at",
        r#"
use omega::language::core::layout;

data ArrayLayout { entries: [FieldEntry; 64]; }
machine ArrayLayout::plan(&mut self, schema: Schema) -> Plan {
    self.entries[0] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 8 },
    };
    Plan { entries: self.entries, entry_count: 1,
           size_fixed: 16, size_is_dynamic: false, align: 2 }
}
data Samples { values: [u16; 3]; }
data Main { }
machine Main::main(&mut self) { }
"#,
    );
    let checked = compile_to_checked(&main_path, None).expect("fixed array should reflect");
    let report = compute_layout_plan(&checked.typed, "ArrayLayout::plan", "Samples")
        .expect("one At placement should admit the complete fixed-array extent");
    assert_eq!(report.offsets, Some(vec![8]));
    assert_eq!(report.size, Some(16));
    assert_eq!(report.align, 2);
}

#[test]
fn nested_fixed_primitive_arrays_remain_one_repeated_at_field() {
    let main_path = write_program(
        "nested-fixed-array-at",
        r#"
use omega::language::core::layout;

data ArrayLayout { entries: [FieldEntry; 64]; }
machine ArrayLayout::plan(&mut self, schema: Schema) -> Plan {
    self.entries[0] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 8 },
    };
    Plan { entries: self.entries, entry_count: 1,
           size_fixed: 16, size_is_dynamic: false, align: 2 }
}

data Samples { values: [[u16; 2]; 2]; }
data Main { }
machine Main::main(&mut self) { }
"#,
    );
    let checked = compile_to_checked(&main_path, None).expect("nested fixed array should reflect");
    let report = compute_layout_plan(&checked.typed, "ArrayLayout::plan", "Samples")
        .expect("one At placement should admit the complete nested-array extent");
    assert_eq!(report.offsets, Some(vec![8]));
    assert_eq!(report.size, Some(16));
    assert_eq!(report.align, 2);
}

#[test]
fn fixed_records_are_reflected_as_one_nested_at_field() {
    let main_path = write_program(
        "fixed-record-at",
        r#"
use omega::language::core::layout;

data RecordLayout { entries: [FieldEntry; 64]; }
machine RecordLayout::plan(&mut self, schema: Schema) -> Plan {
    self.entries[0] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 8 },
    };
    Plan { entries: self.entries, entry_count: 1,
           size_fixed: 24, size_is_dynamic: false, align: 4 }
}

data Pair { low: u8; high: u32; }
data Samples { pair: Pair; }
data Main { }
machine Main::main(&mut self) { }
"#,
    );
    let checked = compile_to_checked(&main_path, None).expect("fixed record should reflect");
    let report = compute_layout_plan(&checked.typed, "RecordLayout::plan", "Samples")
        .expect("one At placement should admit the complete fixed-record extent");
    assert_eq!(report.offsets, Some(vec![8]));
    assert_eq!(report.size, Some(24));
    assert_eq!(report.align, 4);

    let mut bytes = [0xa5; 24];
    materialize_aggregate_layout_into(
        &report,
        &[AggregateFieldSchema::new("pair", 8).expect("compiler-derived pair extent")],
        &[AggregateFieldValue::new("pair", [1, 0, 0, 0, 5, 4, 3, 2]).expect("owned pair bytes")],
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

data RecordLayout { entries: [FieldEntry; 64]; }
machine RecordLayout::plan(&mut self, schema: Schema) -> Plan {
    self.entries[0] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 8 },
    };
    Plan { entries: self.entries, entry_count: 1,
           size_fixed: 24, size_is_dynamic: false, align: 4 }
}
data Pair { low: u8; high: u32; }
data Samples { pair: Pair; }
data Main { }
machine Main::main(&mut self) { }
"#,
    );
    let checked = compile_to_checked(&main_path, None).expect("fixed record should reflect");
    let report = compute_layout_plan(&checked.typed, "RecordLayout::plan", "Samples")
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

data RecordLayout { entries: [FieldEntry; 64]; }
machine RecordLayout::plan(&mut self, schema: Schema) -> Plan {
    self.entries[0] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 4 },
    };
    Plan { entries: self.entries, entry_count: 1,
           size_fixed: 12, size_is_dynamic: false, align: 2 }
}
data Pair { low: u16; high: u16; }
data Samples { #7 legacy_pair: Pair; }
data Main { }
machine Main::main(&mut self) { }
"#,
    );
    let legacy =
        compile_to_checked(&legacy_path, None).expect("legacy numbered schema should check");
    let retained = compute_layout_plan(&legacy.typed, "RecordLayout::plan", "Samples")
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
    let renamed =
        compile_to_checked(&renamed_path, None).expect("renamed numbered schema should check");
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

data RecordLayout { entries: [FieldEntry; 64]; }
machine RecordLayout::plan(&mut self, schema: Schema) -> Plan {
    self.entries[0] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 8 },
    };
    Plan { entries: self.entries, entry_count: 1,
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
    let checked = compile_to_checked(&main_path, None).expect("owned record producer should check");
    let report = compute_layout_plan(&checked.typed, "RecordLayout::plan", "Samples")
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
fn source_machine_owned_fixed_array_materializes_through_the_typed_bridge() {
    let main_path = write_program(
        "source-owned-fixed-array",
        r#"
use omega::language::core::layout;

data ArrayLayout { entries: [FieldEntry; 64]; }
machine ArrayLayout::plan(&mut self, schema: Schema) -> Plan {
    self.entries[0] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 4 },
    };
    Plan { entries: self.entries, entry_count: 1,
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
    let checked = compile_to_checked(&main_path, None).expect("owned array producer should check");
    let report = compute_layout_plan(&checked.typed, "ArrayLayout::plan", "Samples")
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

data ArrayLayout { entries: [FieldEntry; 64]; }
machine ArrayLayout::plan(&mut self, schema: Schema) -> Plan {
    self.entries[0] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 12 },
    };
    self.entries[1] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 4 },
    };
    self.entries[2] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 8 },
    };
    Plan { entries: self.entries, entry_count: 3,
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
    let checked = compile_to_checked(&main_path, None).expect("tiled fixed array should check");
    let report = compute_layout_plan(&checked.typed, "ArrayLayout::plan", "Samples")
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

data ArrayLayout { entries: [FieldEntry; 64]; }
machine ArrayLayout::plan(&mut self, schema: Schema) -> Plan {
    self.entries[0] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 8 },
    };
    Plan { entries: self.entries, entry_count: 1,
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
    let checked =
        compile_to_checked(&main_path, None).expect("owned fixed-record array should check");
    let report = compute_layout_plan(&checked.typed, "ArrayLayout::plan", "Samples")
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

data Split { entries: [FieldEntry; 64]; }
machine Split::plan(&mut self, schema: Schema) -> Plan {
    self.entries[0] = FieldEntry {
        key: schema.fields[1].key,
        placement: FieldPlan::At { offset: 16 },
    };
    self.entries[1] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 2 },
    };
    self.entries[2] = FieldEntry {
        key: schema.fields[1].key,
        placement: FieldPlan::At { offset: 8 },
    };
    Plan { entries: self.entries, entry_count: 3,
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
    let checked = compile_to_checked(&main_path, None)
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

    let report = compute_layout_plan(&checked.typed, "Split::plan", "Samples")
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

data Spread { entries: [FieldEntry; 64]; }
machine Spread::plan(&mut self, schema: Schema) -> Plan {
    self.entries[0] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 4 },
    };
    self.entries[1] = FieldEntry {
        key: schema.fields[1].key,
        placement: FieldPlan::At { offset: 12 },
    };
    Plan { entries: self.entries, entry_count: 2,
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
    let checked = compile_to_checked(&main_path, None).expect("erased field producer should check");
    let report = compute_layout_plan(&checked.typed, "Spread::plan", "Certified")
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

data Whole { entries: [FieldEntry; 64]; }
machine Whole::plan(&mut self, schema: Schema) -> Plan {
    self.entries[0] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 4 },
    };
    Plan { entries: self.entries, entry_count: 1,
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
    let checked =
        compile_to_checked(&main_path, None).expect("nested erased field producer should check");
    let report = compute_layout_plan(&checked.typed, "Whole::plan", "Envelope")
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

data Whole { entries: [FieldEntry; 64]; }
machine Whole::plan(&mut self, schema: Schema) -> Plan {
    self.entries[0] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 4 },
    };
    Plan { entries: self.entries, entry_count: 1,
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
    let checked =
        compile_to_checked(&main_path, None).expect("record array with erased fields should check");
    let report = compute_layout_plan(&checked.typed, "Whole::plan", "Batch")
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

data Tiled { entries: [FieldEntry; 64]; }
machine Tiled::plan(&mut self, schema: Schema) -> Plan {
    self.entries[0] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 16 },
    };
    self.entries[1] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 4 },
    };
    Plan { entries: self.entries, entry_count: 2,
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
    let checked = compile_to_checked(&main_path, None)
        .expect("tiled record array with erased fields should check");
    let report = compute_layout_plan(&checked.typed, "Tiled::plan", "Batch")
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

data Whole { entries: [FieldEntry; 64]; }
machine Whole::plan(&mut self, schema: Schema) -> Plan {
    Plan { entries: self.entries, entry_count: 0,
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
    let checked = compile_to_checked(&main_path, None)
        .expect("all-erased owned record should remain a checked semantic value");
    let report = compute_layout_plan(&checked.typed, "Whole::plan", "ProofBox")
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

#[test]
fn source_machine_owned_nested_all_erased_record_is_semantic_and_storage_free() {
    let main_path = write_program(
        "source-owned-nested-all-erased-record",
        r#"
use omega::language::core::layout;

data Whole { entries: [FieldEntry; 64]; }
machine Whole::plan(&mut self, schema: Schema) -> Plan {
    self.entries[0] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 4 },
    };
    Plan { entries: self.entries, entry_count: 1,
           size_fixed: 12, size_is_dynamic: false, align: 4 }
}
data Evidence { case Only; }
data ProofBox { proof [erased]: Evidence; }
data Envelope { tag: u32; evidence: ProofBox; }
machine make_envelope() -> Envelope {
    Envelope {
        tag: 16909060,
        evidence: ProofBox { proof: Evidence::Only },
    }
}
data Main { }
machine Main::main(&mut self) { }
"#,
    );
    let checked = compile_to_checked(&main_path, None)
        .expect("nested all-erased record should remain semantically checked");
    let report = compute_layout_plan(&checked.typed, "Whole::plan", "Envelope")
        .expect("only the physically relevant scalar should require placement");
    assert_eq!(report.entries.len(), 1);
    let mut bytes = [0xa5; 12];
    evaluate_and_materialize_typed_owned_layout_into(
        &checked.typed,
        "make_envelope",
        "Envelope",
        &report,
        ByteOrder::LittleEndian,
        &mut bytes,
    )
    .expect("nested erased-only content should contribute no bytes");
    assert_eq!(&bytes[4..8], &[4, 3, 2, 1]);
    assert!(bytes[..4].iter().chain(&bytes[8..]).all(|byte| *byte == 0));

    let missing_nested_evidence = BuildTimeValue::Struct {
        type_name: "Envelope".to_owned(),
        fields: vec![
            ("tag".to_owned(), BuildTimeValue::Int(16909060)),
            (
                "evidence".to_owned(),
                BuildTimeValue::Struct {
                    type_name: "ProofBox".to_owned(),
                    fields: Vec::new(),
                },
            ),
        ],
    };
    let mut unchanged = [0x5a; 12];
    let error = materialize_typed_owned_layout_into(
        &checked.typed,
        "Envelope",
        &report,
        &missing_nested_evidence,
        ByteOrder::LittleEndian,
        &mut unchanged,
    )
    .expect_err("storage-free nested evidence must remain semantically mandatory");
    assert!(error.0.contains("0 fields, expected 1"));
    assert_eq!(unchanged, [0x5a; 12]);
}

#[test]
fn source_machine_owned_array_of_erased_records_is_semantic_and_storage_free() {
    let main_path = write_program(
        "source-owned-array-of-erased-records",
        r#"
use omega::language::core::layout;

data Whole { entries: [FieldEntry; 64]; }
machine Whole::plan(&mut self, schema: Schema) -> Plan {
    self.entries[0] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 4 },
    };
    Plan { entries: self.entries, entry_count: 1,
           size_fixed: 12, size_is_dynamic: false, align: 4 }
}
data Evidence { case Only; }
data ProofBox { proof [erased]: Evidence; }
data Envelope { tag: u32; evidence: [ProofBox; 2]; }
machine make_envelope() -> Envelope {
    let evidence: [ProofBox; 2];
    evidence[0] = ProofBox { proof: Evidence::Only };
    evidence[1] = ProofBox { proof: Evidence::Only };
    Envelope {
        tag: 16909060,
        evidence: evidence,
    }
}
data Main { }
machine Main::main(&mut self) { }
"#,
    );
    let checked = compile_to_checked(&main_path, None)
        .expect("an array of erased-only records should remain semantically checked");
    let report = compute_layout_plan(&checked.typed, "Whole::plan", "Envelope")
        .expect("only the physically relevant scalar should require placement");
    assert_eq!(report.entries.len(), 1);
    let mut bytes = [0xa5; 12];
    evaluate_and_materialize_typed_owned_layout_into(
        &checked.typed,
        "make_envelope",
        "Envelope",
        &report,
        ByteOrder::LittleEndian,
        &mut bytes,
    )
    .expect("repeated erased-only content should contribute no bytes");
    assert_eq!(&bytes[4..8], &[4, 3, 2, 1]);
    assert!(bytes[..4].iter().chain(&bytes[8..]).all(|byte| *byte == 0));

    let malformed_repeated_evidence = BuildTimeValue::Struct {
        type_name: "Envelope".to_owned(),
        fields: vec![
            ("tag".to_owned(), BuildTimeValue::Int(16909060)),
            (
                "evidence".to_owned(),
                BuildTimeValue::Array(vec![
                    BuildTimeValue::Struct {
                        type_name: "ProofBox".to_owned(),
                        fields: vec![(
                            "proof".to_owned(),
                            BuildTimeValue::Case {
                                variant: "Only".to_owned(),
                                payload: Vec::new(),
                            },
                        )],
                    },
                    BuildTimeValue::Struct {
                        type_name: "ProofBox".to_owned(),
                        fields: Vec::new(),
                    },
                ]),
            ),
        ],
    };
    let mut unchanged = [0x5a; 12];
    let error = materialize_typed_owned_layout_into(
        &checked.typed,
        "Envelope",
        &report,
        &malformed_repeated_evidence,
        ByteOrder::LittleEndian,
        &mut unchanged,
    )
    .expect_err("every storage-free repeated element must remain semantically complete");
    assert!(error.0.contains("0 fields, expected 1"));
    assert_eq!(unchanged, [0x5a; 12]);
}

#[test]
fn typed_owned_unsigned_values_reject_negative_structured_carriers_atomically() {
    let main_path = write_program(
        "typed-owned-negative-u64",
        r#"
use omega::language::core::layout;

data Whole { entries: [FieldEntry; 64]; }
machine Whole::plan(&mut self, schema: Schema) -> Plan {
    self.entries[0] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 0 },
    };
    Plan { entries: self.entries, entry_count: 1,
           size_fixed: 8, size_is_dynamic: false, align: 8 }
}
data Samples { value: u64; }
data Main { }
machine Main::main(&mut self) { }
"#,
    );
    let checked = compile_to_checked(&main_path, None).expect("u64 schema should check");
    let report = compute_layout_plan(&checked.typed, "Whole::plan", "Samples")
        .expect("u64 should have one whole-field placement");
    let value = BuildTimeValue::Struct {
        type_name: "Samples".to_owned(),
        fields: vec![("value".to_owned(), BuildTimeValue::Int(-1))],
    };
    let mut unchanged = [0x5a; 8];
    let error = materialize_typed_owned_layout_into(
        &checked.typed,
        "Samples",
        &report,
        &value,
        ByteOrder::LittleEndian,
        &mut unchanged,
    )
    .expect_err("negative structured carrier must not inhabit u64");
    assert!(error.0.contains("outside `u64`"));
    assert_eq!(unchanged, [0x5a; 8]);
}

#[test]
fn fixed_primitive_arrays_reject_scalar_bit_placement() {
    let main_path = write_program(
        "fixed-array-bits",
        r#"
use omega::language::core::layout;

data ArrayBits { entries: [FieldEntry; 64]; }
machine ArrayBits::plan(&mut self, schema: Schema) -> Plan {
    self.entries[0] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::Bits {
            container: 0, container_width: 64,
            destination_lsb: 0, source_lsb: 0, width: 48,
        },
    };
    Plan { entries: self.entries, entry_count: 1,
           size_fixed: 8, size_is_dynamic: false, align: 8 }
}
data Samples { values: [u16; 3]; }
data Main { }
machine Main::main(&mut self) { }
"#,
    );
    let checked = compile_to_checked(&main_path, None).expect("program should type");
    let error = compute_layout_plan(&checked.typed, "ArrayBits::plan", "Samples")
        .expect_err("aggregate bit placement must stay outside the fixed-array At slice");
    assert!(error.contains("aggregate fields support only `At` placement"));
}

#[test]
fn effectful_policies_are_rejected_at_the_gate() {
    let main_path = write_program(
        "effectful-policy",
        r#"
data FieldKind { case Scalar; case Text; case Nested; case Repeated; }
data SchemaField { key: u64; size: u64 [0..=4096]; align: u64 [1..=16]; number: i64; kind: FieldKind; }
data Schema { fields: [SchemaField; 32]; field_count: u64 [0..=32]; }
data FieldPlan { case At(offset: u64); case Skip; }
data FieldEntry { key: u64; placement: FieldPlan; }
data Plan { entries: [FieldEntry; 64]; entry_count: u64; size_fixed: u64; size_is_dynamic: bool; align: u64; }
boundary trait Console { machine write(code: i64); }
data Chatty { console: Console; entries: [FieldEntry; 64]; }
machine Chatty::plan(&mut self, schema: Schema) -> Plan {
    self.console.write(1);
    Plan { entries: self.entries, entry_count: schema.field_count,
           size_fixed: 0, size_is_dynamic: true, align: 1 }
}
data Simple { value: i32; }
data Main { }
machine Main::main(&mut self) { }
"#,
    );
    let checked = compile_to_checked(&main_path, None).expect("effectful program should compile");
    let error = compute_layout_plan(&checked.typed, "Chatty::plan", "Simple")
        .expect_err("an effectful policy must be rejected");
    assert!(
        error.contains("not build-time admissible") && error.contains("service reach [Console]"),
        "expected the normalized service-reach gate to reject the policy, got: {error}"
    );
}

#[test]
fn overlapping_plans_are_rejected_by_validation() {
    let main_path = write_program(
        "overlap-policy",
        r#"
data FieldKind { case Scalar; case Text; case Nested; case Repeated; }
data SchemaField { key: u64; size: u64 [0..=4096]; align: u64 [1..=16]; number: i64; kind: FieldKind; }
data Schema { fields: [SchemaField; 32]; field_count: u64 [0..=32]; }
data FieldPlan { case At(offset: u64); case Skip; }
data FieldEntry { key: u64; placement: FieldPlan; }
data Plan { entries: [FieldEntry; 64]; entry_count: u64; size_fixed: u64; size_is_dynamic: bool; align: u64; }
data Overlapper { entries: [FieldEntry; 64]; }
machine Overlapper::plan(&mut self, schema: Schema) -> Plan {
    self.entries[0] = FieldEntry { key: schema.fields[0].key, placement: FieldPlan::At { offset: 0 } };
    self.entries[1] = FieldEntry { key: schema.fields[1].key, placement: FieldPlan::At { offset: 0 } };
    Plan { entries: self.entries, entry_count: schema.field_count,
           size_fixed: 8, size_is_dynamic: false, align: 1 }
}

data Pair { a: i32; b: i32; }
data Main { }
machine Main::main(&mut self) { }
"#,
    );
    let checked = compile_to_checked(&main_path, None).expect("overlap program should compile");
    let error = compute_layout_plan(&checked.typed, "Overlapper::plan", "Pair")
        .expect_err("an overlapping plan must be rejected");
    assert!(
        error.contains("overlap"),
        "expected the overlap diagnostic, got: {error}"
    );
}

#[test]
fn name_keyed_fragments_tile_one_logical_field() {
    let main_path = write_program(
        "fragmented-policy",
        r#"
data FieldKind { case Scalar; case Text; case Nested; case Repeated; }
data SchemaField { key: u64; size: u64 [0..=4096]; align: u64 [1..=16]; number: i64; kind: FieldKind; }
data Schema { fields: [SchemaField; 32]; field_count: u64 [0..=32]; }
data FieldPlan {
    case At(offset: u64);
    case Bits(container: u64, container_width: u64, destination_lsb: u64, source_lsb: u64, width: u64);
}

data FieldEntry { key: u64; placement: FieldPlan; }
data Plan { entries: [FieldEntry; 64]; entry_count: u64; size_fixed: u64; size_is_dynamic: bool; align: u64; }
data SplitAddress { entries: [FieldEntry; 64]; }
machine SplitAddress::plan(&mut self, schema: Schema) -> Plan {
    self.entries[0] = FieldEntry { key: schema.fields[0].key, placement: FieldPlan::Bits {
        container: 0, container_width: 16, destination_lsb: 0, source_lsb: 0, width: 16 } };
    self.entries[1] = FieldEntry { key: schema.fields[0].key, placement: FieldPlan::Bits {
        container: 2, container_width: 16, destination_lsb: 0, source_lsb: 16, width: 16 } };
    self.entries[2] = FieldEntry { key: schema.fields[0].key, placement: FieldPlan::Bits {
        container: 8, container_width: 64, destination_lsb: 0, source_lsb: 32, width: 32 } };
    Plan { entries: self.entries, entry_count: 3, size_fixed: 16, size_is_dynamic: false, align: 1 }
}
data EntryTarget { address: u64; }
data Main { }
machine Main::main(&mut self) { }
"#,
    );
    let checked = compile_to_checked(&main_path, None).expect("fragment policy should compile");
    let report = compute_layout_plan(&checked.typed, "SplitAddress::plan", "EntryTarget")
        .expect("complete fragments should validate");

    assert_eq!(report.offsets, None);
    assert_eq!(report.entries.len(), 3);
    assert!(matches!(
        report.entries[2].placement,
        LayoutPlacementReport::Bits {
            source_lsb: 32,
            width: 32,
            ..
        }
    ));

    let target = RelocationTarget::Entry(
        EntryStubId::from_normalized_identity(0x55aa).expect("normalized entry identity"),
    );
    let symbolic = SymbolicFieldValue::new("address", 64, target).expect("symbolic entry field");
    let materialization = derive_symbolic_materialization(
        &report,
        &[symbolic],
        MaterializationContext {
            consumption: ConsumptionInstant::AfterOmegaHandoff,
            byte_order: ByteOrder::LittleEndian,
            native_pointer_relocation_bits: Some(64),
            placement: psi_layout_plans::PlacementConstraints::unconstrained(
                psi_layout_plans::PlacementPhase::PostHandoff,
            ),
        },
        |_| None,
    )
    .expect("post-handoff split address should derive a writer plan");
    assert_eq!(materialization.actions.len(), 3);
    assert!(
        materialization
            .actions
            .iter()
            .all(|action| matches!(action, MaterializationAction::RuntimeWriter(_)))
    );
}

#[test]
fn integer_at_retains_stored_width_and_extension_interpretation() {
    let main_path = write_program(
        "stored-integer-policy",
        r#"
data FieldKind { case Scalar; case Text; case Nested; case Repeated; }
data SchemaField { key: u64; size: u64 [0..=4096]; align: u64 [1..=16]; number: i64; kind: FieldKind; }
data Schema { fields: [SchemaField; 32]; field_count: u64 [0..=32]; }
data IntegerInterpretation { case Signed; case Unsigned; }
data FieldPlan {
    case At(offset: u64);
    case IntegerAt(offset: u64, stored_width: u64, interpretation: IntegerInterpretation);
}
data FieldEntry { key: u64; placement: FieldPlan; }
data Plan { entries: [FieldEntry; 64]; entry_count: u64; size_fixed: u64; size_is_dynamic: bool; align: u64; }
data ForeignIntegers { entries: [FieldEntry; 64]; }
machine ForeignIntegers::plan(&mut self, schema: Schema) -> Plan {
    self.entries[0] = FieldEntry { key: schema.fields[0].key, placement: FieldPlan::IntegerAt {
        offset: 0, stored_width: 32, interpretation: IntegerInterpretation::Signed } };
    self.entries[1] = FieldEntry { key: schema.fields[1].key, placement: FieldPlan::IntegerAt {
        offset: 4, stored_width: 32, interpretation: IntegerInterpretation::Unsigned } };
    Plan { entries: self.entries, entry_count: 2, size_fixed: 8, size_is_dynamic: false, align: 1 }
}
data PortableStat { seconds: i64; inode: u64; }
data Main { value: ForeignIntegers<PortableStat>; }
machine Main::main(&mut self) { }
"#,
    );
    let mut checked =
        compile_to_checked(&main_path, None).expect("stored integer policy should compile");
    let report = compute_layout_plan(&checked.typed, "ForeignIntegers::plan", "PortableStat")
        .expect("both stored integer ranges fit their semantic carriers");

    assert_eq!(report.offsets, None);
    assert_eq!(report.entries.len(), 2);
    assert!(matches!(
        report.entries[0].placement,
        LayoutPlacementReport::IntegerAt {
            offset: 0,
            stored_width: 32,
            interpretation: IntegerInterpretation::Signed,
        }
    ));
    assert!(matches!(
        report.entries[1].placement,
        LayoutPlacementReport::IntegerAt {
            offset: 4,
            stored_width: 32,
            interpretation: IntegerInterpretation::Unsigned,
        }
    ));

    let recorded = checked
        .typed
        .plan_laid_layouts
        .iter()
        .find(|layout| layout.data_name == "ForeignIntegers<PortableStat>")
        .expect("stored-width geometry should cross the typed plan-laid boundary");
    assert_eq!(recorded.offsets, vec![0, 4]);
    assert_eq!(recorded.integer_fields.len(), 2);
    assert_eq!(recorded.integer_fields[0].field_index, 0);
    assert_eq!(recorded.integer_fields[0].stored_width_bits, 32);
    assert_eq!(
        recorded.integer_fields[0].interpretation,
        IntegerInterpretation::Signed
    );
    assert_eq!(recorded.integer_fields[1].field_index, 1);
    assert_eq!(
        recorded.integer_fields[1].interpretation,
        IntegerInterpretation::Unsigned
    );
    let recorded_index = checked
        .typed
        .plan_laid_layouts
        .iter()
        .position(|layout| layout.data_name == "ForeignIntegers<PortableStat>")
        .expect("stored-width geometry index");
    assert!(!checked.typed.plan_laid_layouts[recorded_index].integer_fields[0].write_is_total);
    checked.typed.plan_laid_layouts[recorded_index].integer_fields[0].write_is_total = true;
    let diagnostics = psi_validation::validate_program(&checked.typed)
        .expect_err("invented total-write capability must fail closed");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("changed its exact stored-integer type capability")
    }));
    checked.typed.plan_laid_layouts[recorded_index].integer_fields[0].write_is_total = false;

    let target = NativeTarget::from_omega_target_name(None).expect("host target");
    let layouts = build_layout_plan(&checked, target).expect("stored integer layout should build");
    let data_layout = layouts
        .data_layouts
        .iter()
        .map(|(_, layout)| layout)
        .find(|layout| layout.name.as_str() == "ForeignIntegers<PortableStat>")
        .expect("synthesized stored-integer record should have a concrete layout");
    let DataShape::Record { fields } = data_layout.shape else {
        panic!("stored-integer layout should remain a record");
    };
    let fields = layouts.fields.span_or_empty(fields);
    assert_eq!(
        fields.iter().map(|field| field.offset).collect::<Vec<_>>(),
        [0, 4]
    );
    assert_eq!(
        layouts
            .stored_integer(fields[0].symbol)
            .expect("signed stored integer metadata")
            .interpretation,
        IntegerInterpretation::Signed
    );
    assert_eq!(
        layouts
            .stored_integer(fields[1].symbol)
            .expect("unsigned stored integer metadata")
            .stored_width_bits,
        32
    );
    assert!(
        !layouts
            .stored_integer(fields[0].symbol)
            .expect("signed stored integer metadata")
            .write_is_total
    );
    assert!(
        !layouts
            .stored_integer(fields[1].symbol)
            .expect("unsigned stored integer metadata")
            .write_is_total
    );

    let values = [
        ScalarFieldValue::new("seconds", 64, (-9_i64) as u64).expect("signed value"),
        ScalarFieldValue::new("inode", 64, 0xfedc_ba98).expect("unsigned value"),
    ];
    let mut bytes = [0xa5_u8; 8];
    materialize_scalar_layout_into(&report, &values, ByteOrder::LittleEndian, &mut bytes)
        .expect("concrete fitting values should use the validated IntegerAt encoding");
    assert_eq!(bytes, [0xf7, 0xff, 0xff, 0xff, 0x98, 0xba, 0xdc, 0xfe]);
    let decoded = decode_scalar_layout(
        &report,
        &[
            ScalarFieldSchema::new("seconds", 64).expect("signed schema"),
            ScalarFieldSchema::new("inode", 64).expect("unsigned schema"),
        ],
        ByteOrder::LittleEndian,
        &bytes,
    )
    .expect("the validated IntegerAt encoding should decode into semantic carriers");
    let decoded = decoded
        .iter()
        .map(|field| (field.field.as_str(), field.value))
        .collect::<std::collections::BTreeMap<_, _>>();
    assert_eq!(decoded["seconds"], (-9_i64) as u64);
    assert_eq!(decoded["inode"], 0xfedc_ba98);
}

#[test]
fn integer_at_retains_total_write_evidence_for_a_bounded_carrier() {
    let canary = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
        "../../../../canaries/pass/layouts/runtime_plan_laid_integer_at_total_write_exit/main.omg",
    );
    let mut checked =
        compile_to_checked(&canary, None).expect("total-write canary should typecheck");
    let recorded_index = checked
        .typed
        .plan_laid_layouts
        .iter()
        .position(|layout| layout.data_name == "SignedByte<PortableByte>")
        .expect("bounded stored integer plan");
    assert!(checked.typed.plan_laid_layouts[recorded_index].integer_fields[0].write_is_total);
    checked.typed.plan_laid_layouts[recorded_index].integer_fields[0].write_is_total = false;
    let diagnostics = psi_validation::validate_program(&checked.typed)
        .expect_err("removed total-write capability must fail closed");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("changed its exact stored-integer type capability")
    }));
    checked.typed.plan_laid_layouts[recorded_index].integer_fields[0].write_is_total = true;
    let target = NativeTarget::from_omega_target_name(None).expect("host target");
    let layouts = build_layout_plan(&checked, target).expect("stored integer layout should build");
    let layout = layouts
        .data_layouts
        .iter()
        .map(|(_, layout)| layout)
        .find(|layout| layout.name.as_str() == "SignedByte<PortableByte>")
        .expect("bounded stored integer layout");
    let DataShape::Record { fields } = layout.shape else {
        panic!("bounded stored integer should remain a record")
    };
    let field = &layouts.fields.span_or_empty(fields)[0];
    assert!(
        layouts
            .stored_integer(field.symbol)
            .expect("stored integer metadata")
            .write_is_total
    );
}

#[test]
fn integer_at_rejects_a_stored_range_the_semantic_carrier_cannot_hold() {
    let main_path = write_program(
        "stored-integer-range-rejection",
        r#"
data FieldKind { case Scalar; case Text; case Nested; case Repeated; }
data SchemaField { key: u64; size: u64 [0..=4096]; align: u64 [1..=16]; number: i64; kind: FieldKind; }
data Schema { fields: [SchemaField; 32]; field_count: u64 [0..=32]; }
data IntegerInterpretation { case Signed; case Unsigned; }
data FieldPlan { case IntegerAt(offset: u64, stored_width: u64, interpretation: IntegerInterpretation); }
data FieldEntry { key: u64; placement: FieldPlan; }
data Plan { entries: [FieldEntry; 64]; entry_count: u64; size_fixed: u64; size_is_dynamic: bool; align: u64; }
data BadInteger { entries: [FieldEntry; 64]; }
machine BadInteger::plan(&mut self, schema: Schema) -> Plan {
    self.entries[0] = FieldEntry { key: schema.fields[0].key, placement: FieldPlan::IntegerAt {
        offset: 0, stored_width: 32, interpretation: IntegerInterpretation::Signed } };
    Plan { entries: self.entries, entry_count: 1, size_fixed: 4, size_is_dynamic: false, align: 1 }
}
data UnsignedOnly { value: u64; }
data Main { }
machine Main::main(&mut self) { }
"#,
    );
    let checked =
        compile_to_checked(&main_path, None).expect("stored integer policy should compile");
    let error = compute_layout_plan(&checked.typed, "BadInteger::plan", "UnsignedOnly")
        .expect_err("a signed stored range cannot totally decode into an unsigned carrier");
    assert!(
        error.contains("cannot totally decode a 32-bit signed integer into `u64`"),
        "unexpected diagnostic: {error}"
    );
}

#[test]
fn bit_placements_use_the_declared_representation_width() {
    let main_path = write_program(
        "compact-bit-policy",
        r#"
data FieldKind { case Scalar; case Text; case Nested; case Repeated; }
data SchemaField { key: u64; size: u64 [0..=4096]; align: u64 [1..=16]; number: i64; kind: FieldKind; }
data Schema { fields: [SchemaField; 32]; field_count: u64 [0..=32]; }
data FieldPlan { case At(offset: u64); case Bits(container: u64, container_width: u64, destination_lsb: u64, source_lsb: u64, width: u64); }
data FieldEntry { key: u64; placement: FieldPlan; }
data Plan { entries: [FieldEntry; 64]; entry_count: u64; size_fixed: u64; size_is_dynamic: bool; align: u64; }
data CompactBits { entries: [FieldEntry; 64]; }
machine CompactBits::plan(&mut self, schema: Schema) -> Plan {
    self.entries[0] = FieldEntry { key: schema.fields[0].key, placement: FieldPlan::Bits {
        container: 0, container_width: 8, destination_lsb: 0, source_lsb: 0, width: 1 } };
    self.entries[1] = FieldEntry { key: schema.fields[1].key, placement: FieldPlan::Bits {
        container: 0, container_width: 8, destination_lsb: 1, source_lsb: 0, width: 3 } };
    Plan { entries: self.entries, entry_count: 2, size_fixed: 1, size_is_dynamic: false, align: 1 }
}
data PackedFlags { present: bool; mode: u8 [0..=7]; }
data Main { }
machine Main::main(&mut self) { }
"#,
    );
    let checked = compile_to_checked(&main_path, None).expect("compact bit policy should compile");
    let report = compute_layout_plan(&checked.typed, "CompactBits::plan", "PackedFlags")
        .expect("bool and range-constrained fields should use their declared bit width");
    assert_eq!(report.size, Some(1));
    assert_eq!(report.entries.len(), 2);
    assert!(matches!(
        report.entries[1].placement,
        LayoutPlacementReport::Bits {
            source_lsb: 0,
            width: 3,
            ..
        }
    ));

    let mut bytes = [0xa5_u8];
    materialize_scalar_layout_into(
        &report,
        &[
            ScalarFieldValue::new("present", 1, 1).expect("present"),
            ScalarFieldValue::new("mode", 3, 5).expect("mode"),
        ],
        ByteOrder::LittleEndian,
        &mut bytes,
    )
    .expect("compiler-validated plan should drive ordinary scalar materialization");
    assert_eq!(bytes, [0b1011]);

    let decoded = decode_scalar_layout(
        &report,
        &[
            ScalarFieldSchema::new("present", 1).expect("present"),
            ScalarFieldSchema::new("mode", 3).expect("mode"),
        ],
        ByteOrder::LittleEndian,
        &bytes,
    )
    .expect("the compiler-validated plan should also drive imported scalar scans");
    assert_eq!(
        decoded
            .iter()
            .map(|field| (field.field.as_str(), field.value))
            .collect::<std::collections::BTreeMap<_, _>>(),
        std::collections::BTreeMap::from([("mode", 5), ("present", 1)])
    );
}

#[test]
fn compact_bit_placements_still_require_complete_source_tiling() {
    let main_path = write_program(
        "compact-bit-gap",
        r#"
data FieldKind { case Scalar; case Text; case Nested; case Repeated; }
data SchemaField { key: u64; size: u64 [0..=4096]; align: u64 [1..=16]; number: i64; kind: FieldKind; }
data Schema { fields: [SchemaField; 32]; field_count: u64 [0..=32]; }
data FieldPlan { case At(offset: u64); case Bits(container: u64, container_width: u64, destination_lsb: u64, source_lsb: u64, width: u64); }
data FieldEntry { key: u64; placement: FieldPlan; }
data Plan { entries: [FieldEntry; 64]; entry_count: u64; size_fixed: u64; size_is_dynamic: bool; align: u64; }
data TooNarrow { entries: [FieldEntry; 64]; }
machine TooNarrow::plan(&mut self, schema: Schema) -> Plan {
    self.entries[0] = FieldEntry { key: schema.fields[0].key, placement: FieldPlan::Bits {
        container: 0, container_width: 8, destination_lsb: 0, source_lsb: 0, width: 2 } };
    Plan { entries: self.entries, entry_count: 1, size_fixed: 1, size_is_dynamic: false, align: 1 }
}
data PackedMode { mode: u8 [0..=7]; }
data Main { }
machine Main::main(&mut self) { }
"#,
    );
    let checked = compile_to_checked(&main_path, None).expect("compact gap should parse");
    let error = compute_layout_plan(&checked.typed, "TooNarrow::plan", "PackedMode")
        .expect_err("a constrained field must still tile every representable bit");
    assert!(
        error.contains("end at bit 2, expected 3"),
        "unexpected diagnostic: {error}"
    );
}

#[test]
fn fragmented_source_gaps_are_rejected() {
    let main_path = write_program(
        "fragment-gap-policy",
        r#"
data FieldKind { case Scalar; case Text; case Nested; case Repeated; }
data SchemaField { key: u64; size: u64 [0..=4096]; align: u64 [1..=16]; number: i64; kind: FieldKind; }
data Schema { fields: [SchemaField; 32]; field_count: u64 [0..=32]; }
data FieldPlan { case At(offset: u64); case Bits(container: u64, container_width: u64, destination_lsb: u64, source_lsb: u64, width: u64); }
data FieldEntry { key: u64; placement: FieldPlan; }
data Plan { entries: [FieldEntry; 64]; entry_count: u64; size_fixed: u64; size_is_dynamic: bool; align: u64; }
data Gap { entries: [FieldEntry; 64]; }
machine Gap::plan(&mut self, schema: Schema) -> Plan {
    self.entries[0] = FieldEntry { key: schema.fields[0].key, placement: FieldPlan::Bits {
        container: 0, container_width: 64, destination_lsb: 0, source_lsb: 0, width: 31 } };
    self.entries[1] = FieldEntry { key: schema.fields[0].key, placement: FieldPlan::Bits {
        container: 8, container_width: 64, destination_lsb: 0, source_lsb: 32, width: 32 } };
    Plan { entries: self.entries, entry_count: 2, size_fixed: 16, size_is_dynamic: false, align: 1 }
}
data EntryTarget { address: u64; }
data Main { }
machine Main::main(&mut self) { }
"#,
    );
    let checked = compile_to_checked(&main_path, None).expect("gap policy should compile");
    let error = compute_layout_plan(&checked.typed, "Gap::plan", "EntryTarget")
        .expect_err("source gaps must reject");
    assert!(
        error.contains("tile exactly"),
        "unexpected diagnostic: {error}"
    );
}

#[test]
fn full_width_unsigned_counts_are_not_reinterpreted_as_signed() {
    let main_path = write_program(
        "full-width-entry-count",
        r#"
data FieldKind { case Scalar; case Text; case Nested; case Repeated; }
data SchemaField { key: u64; size: u64 [0..=4096]; align: u64 [1..=16]; number: i64; kind: FieldKind; }
data Schema { fields: [SchemaField; 32]; field_count: u64 [0..=32]; }
data FieldPlan { case At(offset: u64); }
data FieldEntry { key: u64; placement: FieldPlan; }
data Plan { entries: [FieldEntry; 64]; entry_count: u64; size_fixed: u64; size_is_dynamic: bool; align: u64; }
data Excess { entries: [FieldEntry; 64]; }
machine Excess::plan(&mut self, schema: Schema) -> Plan {
    Plan {
        entries: self.entries,
        entry_count: 18446744073709551615,
        size_fixed: 0,
        size_is_dynamic: false,
        align: 1,
    }
}

data Simple { value: u8; }
data Main { }
machine Main::main(&mut self) { }
"#,
    );
    let checked =
        compile_to_checked(&main_path, None).expect("full-width u64 policy should compile");
    let error = compute_layout_plan(&checked.typed, "Excess::plan", "Simple")
        .expect_err("a full-width entry count must exceed the plan capacity");
    assert!(
        error.contains("entry_count 18446744073709551615 is outside 0..=64"),
        "unexpected diagnostic: {error}"
    );
}

#[test]
fn reflected_schema_exposes_stable_case_identity_without_using_discriminant() {
    let main_path = write_program(
        "stable-case-schema",
        r#"
use omega::language::core::layout;
use omega::language::core::option;

data Choice {
    case #41 First;
    case #7 Second;
    retired #99;
}

data InspectCases { entries: [FieldEntry; 64]; }
machine InspectCases::plan(&mut self, schema: Schema) -> Plan {
    transition schema.cases[0].identity {
        Optional::Some { value } -> selected(value, schema)
        Optional::None -> selected(1, schema)
    }
    state selected(&mut self, identity: u64, schema: Schema) {
        transition schema.retired_case_identity_count == 1
            && schema.retired_case_identities[0] == 99 {
        true -> (Plan {
            entries: self.entries,
            entry_count: 0,
            size_fixed: identity,
            size_is_dynamic: false,
            align: 1
        })
        _ -> (Plan {
            entries: self.entries,
            entry_count: 0,
            size_fixed: 1,
            size_is_dynamic: false,
            align: 1
        })
        }
    }
}

data Main { }
machine Main::main(&mut self) { }
"#,
    );
    let checked = compile_to_checked(&main_path, None).expect("case schema should compile");
    let report = compute_layout_plan(&checked.typed, "InspectCases::plan", "Choice")
        .expect("case identity should reach the build-time Schema value");
    assert_eq!(
        report.size,
        Some(41),
        "the first authored case has runtime discriminant zero, but its reflected stable identity remains #41"
    );
    assert_ne!(report.schema_identity, 0);

    let reordered_path = write_program(
        "stable-case-schema-reordered",
        r#"
use omega::language::core::layout;

data Choice {
    case #7 SecondRenamed;
    case #41 FirstRenamed;
    retired #99;
}

data InspectCases { entries: [FieldEntry; 64]; }
machine InspectCases::plan(&mut self, schema: Schema) -> Plan {
    Plan {
        entries: self.entries,
        entry_count: 0,
        size_fixed: 1,
        size_is_dynamic: false,
        align: 1
    }
}

data Main { }
machine Main::main(&mut self) { }
"#,
    );
    let reordered =
        compile_to_checked(&reordered_path, None).expect("reordered case schema should compile");
    let reordered_report = compute_layout_plan(&reordered.typed, "InspectCases::plan", "Choice")
        .expect("reordered case schema should normalize");
    assert_eq!(
        report.schema_identity, reordered_report.schema_identity,
        "numbered case names and authored order are presentation/runtime-discriminant inputs, not stable schema identity"
    );
}
