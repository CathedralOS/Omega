//! L3 of the LAYOUTS ladder: the zero-codegen plan pipeline, end to end. The
//! pilot policy is the brief's headline claim made real -- THE C ABI AS ~15
//! LINES OF OMEGA: an effect-free `CLayout::plan` machine (round up to the
//! field's alignment, place, track the widest alignment, round the total) is
//! evaluated at BUILD TIME against a compiler-materialized Schema, and the
//! compiler VALIDATES the plan before reporting it. A buggy policy is a
//! compile error, never unsafety -- which is also why the policy's scratch
//! arithmetic may honestly declare Wrapping: plan validation owns soundness.

use omega_compiler::{
    ByteOrder, ConsumptionInstant, EntryStubId, LayoutPlacementReport, MaterializationAction,
    MaterializationContext, RelocationTarget, SymbolicFieldValue, compile_to_checked,
    compute_layout_plan, derive_symbolic_materialization,
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

/// The vocabulary (mirrors omega/language/std/layout.omg) + the CLayout
/// policy + a UEFI-ish schema. Inlined because temp-dir programs cannot
/// resolve `use omega::...` library paths.
const PILOT: &str = r#"
data FieldKind { case Scalar; case Text; case Nested; case Repeated; }
data SchemaField {
    key: i64;
    size: i64 [0..=4096];
    align: i64 [1..=16];
    number: i64;
    kind: FieldKind;
}
data Schema {
    fields: [SchemaField; 32];
    field_count: i64 [0..=32];
}
data FieldPlan {
    case At(offset: i64);
    case Bits(container: i64, container_width: i64, destination_lsb: i64, source_lsb: i64, width: i64);
    case Varint(tag: i64);
    case LengthPrefixed(tag: i64);
}
data FieldEntry { key: i64; placement: FieldPlan; }
data Plan {
    entries: [FieldEntry; 64];
    entry_count: i64;
    size_fixed: i64;
    size_is_dynamic: bool;
    align: i64;
}

// The C layout rule as a build-time Omega machine. Scratch accumulators are
// DECLARED Wrapping (policy-internal arithmetic; the compiler's plan
// validation catches any garbage plan), and the padding uses the modulo form
// ((a - offset % a) % a) so no division is needed.
data CLayout {
    entries: [FieldEntry; 64];
    index: i64 in Wrapping;
    offset: i64 in Wrapping;
    widest: i64 in Wrapping;
    fsize: i64 in Wrapping;
    falign: i64 in Wrapping;
    pad: i64 in Wrapping;
}
machine CLayout::plan(&mut self, schema: Schema) -> Plan {
    self.index = 0;
    self.offset = 0;
    self.widest = 1;
    transition { _ -> place_loop(schema) }

    state place_loop(&mut self, schema: Schema) {
        transition self.index >= 0 && self.index < 32 && self.index < schema.field_count {
            true -> read_field(schema)
            _ -> done(schema)
        }
    }
    state read_field(&mut self, schema: Schema) {
        self.fsize = schema.fields[self.index].size;
        self.falign = schema.fields[self.index].align;
        // C rule: round up to the field's alignment, place, advance.
        self.pad = (self.falign - self.offset % self.falign) % self.falign;
        self.offset = self.offset + self.pad;
        transition self.index >= 0 && self.index < 32 {
            true -> place_field(schema)
            _ -> done(schema)
        }
    }
    state place_field(&mut self, schema: Schema) {
        self.entries[self.index] = FieldEntry {
            key: schema.fields[self.index].key,
            placement: FieldPlan::At { offset: self.offset as i64 },
        };
        self.offset = self.offset + self.fsize;
        transition self.widest < self.falign {
            true -> widen(schema)
            _ -> advance(schema)
        }
    }
    state widen(&mut self, schema: Schema) {
        self.widest = self.falign;
        transition { _ -> advance(schema) }
    }
    state advance(&mut self, schema: Schema) {
        self.index = self.index + 1;
        transition { _ -> place_loop(schema) }
    }
    state done(&mut self, schema: Schema) -> Plan {
        // Round the total size up to the struct alignment (the C tail rule).
        self.pad = (self.widest - self.offset % self.widest) % self.widest;
        Plan {
            entries: self.entries,
            entry_count: schema.field_count as i64,
            size_fixed: (self.offset + self.pad) as i64,
            size_is_dynamic: false,
            align: self.widest as i64,
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
    let checked = compile_to_checked(&canary, None).expect("plan-laid canary should compile");

    // The pipeline recorded the validated plan on the typed trees.
    assert_eq!(checked.typed.plan_laid_layouts.len(), 1);
    let recorded = &checked.typed.plan_laid_layouts[0];
    assert_eq!(recorded.data_name, "Spread16<Gdtish>");
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
fn effectful_policies_are_rejected_at_the_gate() {
    let main_path = write_program(
        "effectful-policy",
        r#"
data FieldKind { case Scalar; case Text; case Nested; case Repeated; }
data SchemaField { key: i64; size: i64 [0..=4096]; align: i64 [1..=16]; number: i64; kind: FieldKind; }
data Schema { fields: [SchemaField; 32]; field_count: i64 [0..=32]; }
data FieldPlan { case At(offset: i64); case Skip; }
data FieldEntry { key: i64; placement: FieldPlan; }
data Plan { entries: [FieldEntry; 64]; entry_count: i64; size_fixed: i64; size_is_dynamic: bool; align: i64; }
boundary trait Console { machine write(code: i64); }
data Chatty { console: Console; entries: [FieldEntry; 64]; }
machine Chatty::plan(&mut self, schema: Schema) -> Plan {
    self.console.write(1);
    Plan { entries: self.entries, entry_count: schema.field_count as i64,
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
data SchemaField { key: i64; size: i64 [0..=4096]; align: i64 [1..=16]; number: i64; kind: FieldKind; }
data Schema { fields: [SchemaField; 32]; field_count: i64 [0..=32]; }
data FieldPlan { case At(offset: i64); case Skip; }
data FieldEntry { key: i64; placement: FieldPlan; }
data Plan { entries: [FieldEntry; 64]; entry_count: i64; size_fixed: i64; size_is_dynamic: bool; align: i64; }
data Overlapper { entries: [FieldEntry; 64]; }
machine Overlapper::plan(&mut self, schema: Schema) -> Plan {
    self.entries[0] = FieldEntry { key: schema.fields[0].key, placement: FieldPlan::At { offset: 0 } };
    self.entries[1] = FieldEntry { key: schema.fields[1].key, placement: FieldPlan::At { offset: 0 } };
    Plan { entries: self.entries, entry_count: schema.field_count as i64,
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
data SchemaField { key: i64; size: i64 [0..=4096]; align: i64 [1..=16]; number: i64; kind: FieldKind; }
data Schema { fields: [SchemaField; 32]; field_count: i64 [0..=32]; }
data FieldPlan {
    case At(offset: i64);
    case Bits(container: i64, container_width: i64, destination_lsb: i64, source_lsb: i64, width: i64);
}
data FieldEntry { key: i64; placement: FieldPlan; }
data Plan { entries: [FieldEntry; 64]; entry_count: i64; size_fixed: i64; size_is_dynamic: bool; align: i64; }
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
            placement: omega_layout_plans::PlacementConstraints::unconstrained(
                omega_layout_plans::PlacementPhase::PostHandoff,
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
fn bit_placements_use_the_declared_representation_width() {
    let main_path = write_program(
        "compact-bit-policy",
        r#"
data FieldKind { case Scalar; case Text; case Nested; case Repeated; }
data SchemaField { key: i64; size: i64 [0..=4096]; align: i64 [1..=16]; number: i64; kind: FieldKind; }
data Schema { fields: [SchemaField; 32]; field_count: i64 [0..=32]; }
data FieldPlan { case At(offset: i64); case Bits(container: i64, container_width: i64, destination_lsb: i64, source_lsb: i64, width: i64); }
data FieldEntry { key: i64; placement: FieldPlan; }
data Plan { entries: [FieldEntry; 64]; entry_count: i64; size_fixed: i64; size_is_dynamic: bool; align: i64; }
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
}

#[test]
fn compact_bit_placements_still_require_complete_source_tiling() {
    let main_path = write_program(
        "compact-bit-gap",
        r#"
data FieldKind { case Scalar; case Text; case Nested; case Repeated; }
data SchemaField { key: i64; size: i64 [0..=4096]; align: i64 [1..=16]; number: i64; kind: FieldKind; }
data Schema { fields: [SchemaField; 32]; field_count: i64 [0..=32]; }
data FieldPlan { case At(offset: i64); case Bits(container: i64, container_width: i64, destination_lsb: i64, source_lsb: i64, width: i64); }
data FieldEntry { key: i64; placement: FieldPlan; }
data Plan { entries: [FieldEntry; 64]; entry_count: i64; size_fixed: i64; size_is_dynamic: bool; align: i64; }
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
data SchemaField { key: i64; size: i64 [0..=4096]; align: i64 [1..=16]; number: i64; kind: FieldKind; }
data Schema { fields: [SchemaField; 32]; field_count: i64 [0..=32]; }
data FieldPlan { case At(offset: i64); case Bits(container: i64, container_width: i64, destination_lsb: i64, source_lsb: i64, width: i64); }
data FieldEntry { key: i64; placement: FieldPlan; }
data Plan { entries: [FieldEntry; 64]; entry_count: i64; size_fixed: i64; size_is_dynamic: bool; align: i64; }
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
