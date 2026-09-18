use super::{package_inputs_with_standard_library, write_program};
use crate::fixture_roster;
use build_time_evaluation::{
    BuildTimeValue, compute_layout_plan, evaluate_and_materialize_typed_owned_layout_into,
    materialize_typed_owned_layout_into,
};
use compiler::{CheckedCompileRequest, compile_to_checked};
use layout::{DataShape, build_layout_plan};
use layout_plans::{
    ByteOrder, ConsumptionInstant, EntryStubId, IntegerInterpretation, LayoutPlacementReport,
    MaterializationAction, MaterializationContext, RelocationTarget, ScalarFieldSchema,
    ScalarFieldValue, SymbolicFieldValue, decode_scalar_layout, derive_symbolic_materialization,
    materialize_scalar_layout_into,
};
use std::fs;
use std::path::Path;
use target::NativeTarget;

#[test]
fn source_machine_owned_nested_all_erased_record_is_semantic_and_storage_free() {
    let main_path = write_program(
        "source-owned-nested-all-erased-record",
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
    let checked = compile_to_checked(CheckedCompileRequest::new(&main_path, None))
        .expect("nested all-erased record should remain semantically checked");
    let report = compute_layout_plan(&checked.typed, "Whole::plan", "Envelope", None)
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

data Whole { }
machine Whole::plan(&mut self, schema: Schema) -> Plan {
    let mut entries: [FieldEntry; 64];
    entries[0] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 4 },
    };
    Plan { entries: entries, entry_count: 1,
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
    let checked = compile_to_checked(CheckedCompileRequest::new(&main_path, None))
        .expect("an array of erased-only records should remain semantically checked");
    let report = compute_layout_plan(&checked.typed, "Whole::plan", "Envelope", None)
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

data Whole { }
machine Whole::plan(&mut self, schema: Schema) -> Plan {
    let mut entries: [FieldEntry; 64];
    entries[0] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 0 },
    };
    Plan { entries: entries, entry_count: 1,
           size_fixed: 8, size_is_dynamic: false, align: 8 }
}
data Samples { value: u64; }
data Main { }
machine Main::main(&mut self) { }
"#,
    );
    let checked = compile_to_checked(CheckedCompileRequest::new(&main_path, None))
        .expect("u64 schema should check");
    let report = compute_layout_plan(&checked.typed, "Whole::plan", "Samples", None)
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

data ArrayBits { }
machine ArrayBits::plan(&mut self, schema: Schema) -> Plan {
    let mut entries: [FieldEntry; 64];
    entries[0] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::Bits {
            container: 0, container_width: 64,
            destination_lsb: 0, source_lsb: 0, width: 48,
        },
    };
    Plan { entries: entries, entry_count: 1,
           size_fixed: 8, size_is_dynamic: false, align: 8 }
}
data Samples { values: [u16; 3]; }
data Main { }
machine Main::main(&mut self) { }
"#,
    );
    let checked = compile_to_checked(CheckedCompileRequest::new(&main_path, None))
        .expect("program should type");
    let error = compute_layout_plan(&checked.typed, "ArrayBits::plan", "Samples", None)
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
data Chatty { console: Console; }
machine Chatty::plan(&mut self, schema: Schema) -> Plan {
    let entries: [FieldEntry; 64];
    self.console.write(1);
    Plan { entries: entries, entry_count: schema.field_count,
           size_fixed: 0, size_is_dynamic: true, align: 1 }
}
data Simple { value: i32; }
data Main { }
machine Main::main(&mut self) { }
"#,
    );
    let checked = compile_to_checked(CheckedCompileRequest::new(&main_path, None))
        .expect("effectful program should compile");
    let error = compute_layout_plan(&checked.typed, "Chatty::plan", "Simple", None)
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
data Overlapper { }
machine Overlapper::plan(&mut self, schema: Schema) -> Plan {
    let mut entries: [FieldEntry; 64];
    entries[0] = FieldEntry { key: schema.fields[0].key, placement: FieldPlan::At { offset: 0 } };
    entries[1] = FieldEntry { key: schema.fields[1].key, placement: FieldPlan::At { offset: 0 } };
    Plan { entries: entries, entry_count: schema.field_count,
           size_fixed: 8, size_is_dynamic: false, align: 1 }
}

data Pair { a: i32; b: i32; }
data Main { }
machine Main::main(&mut self) { }
"#,
    );
    let checked = compile_to_checked(CheckedCompileRequest::new(&main_path, None))
        .expect("overlap program should compile");
    let error = compute_layout_plan(&checked.typed, "Overlapper::plan", "Pair", None)
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
data SplitAddress { }
machine SplitAddress::plan(&mut self, schema: Schema) -> Plan {
    let mut entries: [FieldEntry; 64];
    entries[0] = FieldEntry { key: schema.fields[0].key, placement: FieldPlan::Bits {
        container: 0, container_width: 16, destination_lsb: 0, source_lsb: 0, width: 16 } };
    entries[1] = FieldEntry { key: schema.fields[0].key, placement: FieldPlan::Bits {
        container: 2, container_width: 16, destination_lsb: 0, source_lsb: 16, width: 16 } };
    entries[2] = FieldEntry { key: schema.fields[0].key, placement: FieldPlan::Bits {
        container: 8, container_width: 64, destination_lsb: 0, source_lsb: 32, width: 32 } };
    Plan { entries: entries, entry_count: 3, size_fixed: 16, size_is_dynamic: false, align: 1 }
}
data EntryTarget { address: u64; }
data Main { }
machine Main::main(&mut self) { }
"#,
    );
    let checked = compile_to_checked(CheckedCompileRequest::new(&main_path, None))
        .expect("fragment policy should compile");
    let report = compute_layout_plan(&checked.typed, "SplitAddress::plan", "EntryTarget", None)
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
            placement: layout_plans::PlacementConstraints::unconstrained(
                layout_plans::PlacementPhase::PostHandoff,
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
data ForeignIntegers { }
machine ForeignIntegers::plan(&mut self, schema: Schema) -> Plan {
    let mut entries: [FieldEntry; 64];
    entries[0] = FieldEntry { key: schema.fields[0].key, placement: FieldPlan::IntegerAt {
        offset: 0, stored_width: 32, interpretation: IntegerInterpretation::Signed } };
    entries[1] = FieldEntry { key: schema.fields[1].key, placement: FieldPlan::IntegerAt {
        offset: 4, stored_width: 32, interpretation: IntegerInterpretation::Unsigned } };
    Plan { entries: entries, entry_count: 2, size_fixed: 8, size_is_dynamic: false, align: 1 }
}
data PortableStat { seconds: i64; inode: u64; }
data Main { value: ForeignIntegers<PortableStat>; }
machine Main::main(&mut self) { }
"#,
    );
    let mut checked = compile_to_checked(CheckedCompileRequest::new(&main_path, None))
        .expect("stored integer policy should compile")
        .into_program();
    let report = compute_layout_plan(
        &checked.typed,
        "ForeignIntegers::plan",
        "PortableStat",
        None,
    )
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
    let diagnostics = validation::validate_program(&checked.typed)
        .expect_err("invented total-write capability must fail closed");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("changed its exact stored-integer type capability")
    }));
    checked.typed.plan_laid_layouts[recorded_index].integer_fields[0].write_is_total = false;

    let target = NativeTarget::from_omega_target_name(None).expect("host target");
    let layouts =
        build_layout_plan(&checked, target, &[]).expect("stored integer layout should build");
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
    let canary = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("compiler crate should live below the repository root")
        .join("tests/omega/pass")
        .join(fixture_roster::RUNTIME_PLAN_LAID_INTEGER_AT_TOTAL_WRITE_EXIT)
        .join("main.omg");
    let mut checked = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(package_inputs_with_standard_library(&canary)),
        ..CheckedCompileRequest::new(&canary, None)
    })
    .expect("total-write canary should typecheck")
    .into_program();
    let recorded_index = checked
        .typed
        .plan_laid_layouts
        .iter()
        .position(|layout| layout.data_name == "SignedByte<PortableByte>")
        .expect("bounded stored integer plan");
    assert!(checked.typed.plan_laid_layouts[recorded_index].integer_fields[0].write_is_total);
    checked.typed.plan_laid_layouts[recorded_index].integer_fields[0].write_is_total = false;
    let diagnostics = validation::validate_program(&checked.typed)
        .expect_err("removed total-write capability must fail closed");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("changed its exact stored-integer type capability")
    }));
    checked.typed.plan_laid_layouts[recorded_index].integer_fields[0].write_is_total = true;
    let target = NativeTarget::from_omega_target_name(None).expect("host target");
    let layouts =
        build_layout_plan(&checked, target, &[]).expect("stored integer layout should build");
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
data BadInteger { }
machine BadInteger::plan(&mut self, schema: Schema) -> Plan {
    let mut entries: [FieldEntry; 64];
    entries[0] = FieldEntry { key: schema.fields[0].key, placement: FieldPlan::IntegerAt {
        offset: 0, stored_width: 32, interpretation: IntegerInterpretation::Signed } };
    Plan { entries: entries, entry_count: 1, size_fixed: 4, size_is_dynamic: false, align: 1 }
}
data UnsignedOnly { value: u64; }
data Main { }
machine Main::main(&mut self) { }
"#,
    );
    let checked = compile_to_checked(CheckedCompileRequest::new(&main_path, None))
        .expect("stored integer policy should compile");
    let error = compute_layout_plan(&checked.typed, "BadInteger::plan", "UnsignedOnly", None)
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
data CompactBits { }
machine CompactBits::plan(&mut self, schema: Schema) -> Plan {
    let mut entries: [FieldEntry; 64];
    entries[0] = FieldEntry { key: schema.fields[0].key, placement: FieldPlan::Bits {
        container: 0, container_width: 8, destination_lsb: 0, source_lsb: 0, width: 1 } };
    entries[1] = FieldEntry { key: schema.fields[1].key, placement: FieldPlan::Bits {
        container: 0, container_width: 8, destination_lsb: 1, source_lsb: 0, width: 3 } };
    Plan { entries: entries, entry_count: 2, size_fixed: 1, size_is_dynamic: false, align: 1 }
}
data PackedFlags { present: bool; mode: u8 [0..=7]; }
data Main { }
machine Main::main(&mut self) { }
"#,
    );
    let checked = compile_to_checked(CheckedCompileRequest::new(&main_path, None))
        .expect("compact bit policy should compile");
    let report = compute_layout_plan(&checked.typed, "CompactBits::plan", "PackedFlags", None)
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

    let fractional_source = fs::read_to_string(&main_path)
        .expect("literal range fixture")
        .replace("mode: u8 [0..=7]", "mode: u8 [0..=(7 / 2) * 2 + 1]");
    for width in [3, 4] {
        let source = fractional_source.replace("width: 3 }", &format!("width: {width} }}"));
        let main_path = write_program(&format!("fractional-bit-policy-{width}"), &source);
        let checked = compile_to_checked(CheckedCompileRequest::new(&main_path, None))
            .expect("the exact anonymous endpoint is the integer eight");
        let report = compute_layout_plan(&checked.typed, "CompactBits::plan", "PackedFlags", None);
        if width == 3 {
            let error = report.expect_err("eight cannot fit in a three-bit field");
            assert!(error.contains("end at bit 3, expected 4"), "{error}");
        } else {
            let report = report.expect("four bits represent the exact declared range");
            assert!(matches!(
                report.entries[1].placement,
                LayoutPlacementReport::Bits { width: 4, .. }
            ));
            let mut bytes = [0_u8];
            materialize_scalar_layout_into(
                &report,
                &[
                    ScalarFieldValue::new("present", 1, 1).expect("present"),
                    ScalarFieldValue::new("mode", 4, 8).expect("mode"),
                ],
                ByteOrder::LittleEndian,
                &mut bytes,
            )
            .expect("the exact endpoint fits the validated layout");
            assert_eq!(bytes, [0b10001]);
        }
    }
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
data TooNarrow { }
machine TooNarrow::plan(&mut self, schema: Schema) -> Plan {
    let mut entries: [FieldEntry; 64];
    entries[0] = FieldEntry { key: schema.fields[0].key, placement: FieldPlan::Bits {
        container: 0, container_width: 8, destination_lsb: 0, source_lsb: 0, width: 2 } };
    Plan { entries: entries, entry_count: 1, size_fixed: 1, size_is_dynamic: false, align: 1 }
}
data PackedMode { mode: u8 [0..=7]; }
data Main { }
machine Main::main(&mut self) { }
"#,
    );
    let checked = compile_to_checked(CheckedCompileRequest::new(&main_path, None))
        .expect("compact gap should parse");
    let error = compute_layout_plan(&checked.typed, "TooNarrow::plan", "PackedMode", None)
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
data Gap { }
machine Gap::plan(&mut self, schema: Schema) -> Plan {
    let mut entries: [FieldEntry; 64];
    entries[0] = FieldEntry { key: schema.fields[0].key, placement: FieldPlan::Bits {
        container: 0, container_width: 64, destination_lsb: 0, source_lsb: 0, width: 31 } };
    entries[1] = FieldEntry { key: schema.fields[0].key, placement: FieldPlan::Bits {
        container: 8, container_width: 64, destination_lsb: 0, source_lsb: 32, width: 32 } };
    Plan { entries: entries, entry_count: 2, size_fixed: 16, size_is_dynamic: false, align: 1 }
}
data EntryTarget { address: u64; }
data Main { }
machine Main::main(&mut self) { }
"#,
    );
    let checked = compile_to_checked(CheckedCompileRequest::new(&main_path, None))
        .expect("gap policy should compile");
    let error = compute_layout_plan(&checked.typed, "Gap::plan", "EntryTarget", None)
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
data Excess { }
machine Excess::plan(&mut self, schema: Schema) -> Plan {
    let entries: [FieldEntry; 64];
    Plan {
        entries: entries,
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
    let checked = compile_to_checked(CheckedCompileRequest::new(&main_path, None))
        .expect("full-width u64 policy should compile");
    let error = compute_layout_plan(&checked.typed, "Excess::plan", "Simple", None)
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

data InspectCases { }
machine InspectCases::plan(&mut self, schema: Schema) -> Plan {
    transition schema.cases[0].identity {
        Optional::Some { value } -> selected(value, schema)
        Optional::None -> selected(1, schema)
    }
    state selected(&mut self, identity: u64, schema: Schema) {
        let entries: [FieldEntry; 64];
        transition schema.retired_case_identity_count == 1
            && schema.retired_case_identities[0] == 99 {
        true -> (Plan {
            entries: entries,
            entry_count: 0,
            size_fixed: identity,
            size_is_dynamic: false,
            align: 1
        })
        _ -> (Plan {
            entries: entries,
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
    let checked = compile_to_checked(CheckedCompileRequest::new(&main_path, None))
        .expect("case schema should compile");
    let report = compute_layout_plan(&checked.typed, "InspectCases::plan", "Choice", None)
        .expect("case identity should reach the build-time Schema value");
    assert_eq!(
        report.size,
        Some(41),
        "the first authored case has runtime discriminant zero, but its reflected stable identity remains #41"
    );
    assert_ne!(report.schema_report_fingerprint, 0);

    let reordered_path = write_program(
        "stable-case-schema-reordered",
        r#"
use omega::language::core::layout;

data Choice {
    case #7 SecondRenamed;
    case #41 FirstRenamed;
    retired #99;
}

data InspectCases { }
machine InspectCases::plan(&mut self, schema: Schema) -> Plan {
    let entries: [FieldEntry; 64];
    Plan {
        entries: entries,
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
    let reordered = compile_to_checked(CheckedCompileRequest::new(&reordered_path, None))
        .expect("reordered case schema should compile");
    let reordered_report =
        compute_layout_plan(&reordered.typed, "InspectCases::plan", "Choice", None)
            .expect("reordered case schema should normalize");
    assert_eq!(
        report.schema_report_fingerprint, reordered_report.schema_report_fingerprint,
        "numbered case names and authored order are presentation/runtime-discriminant inputs, not stable schema identity"
    );
}

#[test]
fn mixed_scalar_placements_materialize_beside_aggregate_fields() {
    let main_path = write_program(
        "mixed-placement-policy",
        r#"
data FieldKind { case Scalar; case Text; case Nested; case Repeated; }
data SchemaField { key: u64; size: u64 [0..=4096]; align: u64 [1..=16]; number: i64; kind: FieldKind; }
data Schema { fields: [SchemaField; 32]; field_count: u64 [0..=32]; }
data IntegerInterpretation { case Signed; case Unsigned; }
data FieldPlan {
    case At(offset: u64);
    case IntegerAt(offset: u64, stored_width: u64, interpretation: IntegerInterpretation);
    case Bits(container: u64, container_width: u64, destination_lsb: u64, source_lsb: u64, width: u64);
}
data FieldEntry { key: u64; placement: FieldPlan; }
data Plan { entries: [FieldEntry; 64]; entry_count: u64; size_fixed: u64; size_is_dynamic: bool; align: u64; }
data PackedFrame { }
machine PackedFrame::plan(&mut self, schema: Schema) -> Plan {
    let mut entries: [FieldEntry; 64];
    entries[0] = FieldEntry { key: schema.fields[0].key, placement: FieldPlan::IntegerAt {
        offset: 0, stored_width: 16, interpretation: IntegerInterpretation::Unsigned } };
    entries[1] = FieldEntry { key: schema.fields[1].key, placement: FieldPlan::Bits {
        container: 2, container_width: 16, destination_lsb: 0, source_lsb: 0, width: 10 } };
    entries[2] = FieldEntry { key: schema.fields[2].key, placement: FieldPlan::At { offset: 4 } };
    entries[3] = FieldEntry { key: schema.fields[2].key, placement: FieldPlan::At { offset: 6 } };
    entries[4] = FieldEntry { key: schema.fields[3].key, placement: FieldPlan::At { offset: 8 } };
    Plan { entries: entries, entry_count: 5, size_fixed: 12, size_is_dynamic: false, align: 4 }
}
data Cell { lo: u8; hi: u8; }
data Frame { id: u32; packed: u16 [0..=1023]; cells: [Cell; 2]; tag: u8; }
machine make_frame() -> Frame {
    let mut cells: [Cell; 2];
    cells[0] = Cell { lo: 1, hi: 2 };
    cells[1] = Cell { lo: 3, hi: 4 };
    Frame { id: 4660, packed: 677, cells: cells, tag: 171 }
}
data Main { }
machine Main::main(&mut self) { }
"#,
    );
    let checked = compile_to_checked(CheckedCompileRequest::new(&main_path, None))
        .expect("mixed placement policy should compile");
    let report = compute_layout_plan(&checked.typed, "PackedFrame::plan", "Frame", None)
        .expect("a plan mixing scalar and aggregate placements should validate");
    assert_eq!(report.entries.len(), 5);
    assert_eq!(report.offsets, None);
    assert_eq!(report.size, Some(12));

    let mut little = [0xa5; 12];
    evaluate_and_materialize_typed_owned_layout_into(
        &checked.typed,
        "make_frame",
        "Frame",
        &report,
        ByteOrder::LittleEndian,
        &mut little,
    )
    .expect("scalar placements should replay beside aggregate extents");
    assert_eq!(little, [0x34, 0x12, 0xa5, 0x02, 1, 2, 3, 4, 0xab, 0, 0, 0]);

    let mut big = [0xa5; 12];
    evaluate_and_materialize_typed_owned_layout_into(
        &checked.typed,
        "make_frame",
        "Frame",
        &report,
        ByteOrder::BigEndian,
        &mut big,
    )
    .expect("the same plan should materialize big-endian");
    assert_eq!(big, [0x12, 0x34, 0x02, 0xa5, 1, 2, 3, 4, 0xab, 0, 0, 0]);
}
