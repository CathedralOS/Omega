//! L3 of the LAYOUTS ladder: the zero-codegen plan pipeline, end to end. The
//! pilot policy is the brief's headline claim made real -- THE C ABI AS ~15
//! LINES OF OMEGA: an effect-free `CLayout::plan` machine (round up to the
//! field's alignment, place, track the widest alignment, round the total) is
//! evaluated at BUILD TIME against a compiler-materialized Schema, and the
//! compiler VALIDATES the plan before reporting it. A buggy policy is a
//! compile error, never unsafety -- which is also why the policy's scratch
//! arithmetic may honestly declare Wrapping: plan validation owns soundness.

use omega_compiler::{compile_to_checked, compute_layout_plan};
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
data SchemaField {
    size: i64 [0..=4096];
    align: i64 [1..=16];
    number: i64;
}
data Schema {
    fields: [SchemaField; 32];
    field_count: i64 [0..=32];
}
data FieldPlan {
    case At(offset: i64);
    case Bits(container: i64, container_width: i64, lsb: i64, width: i64);
    case Varint(tag: i64);
    case LengthPrefixed(tag: i64);
}
data Plan {
    fields: [FieldPlan; 32];
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
    plans: [FieldPlan; 32];
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
        self.plans[self.index] = FieldPlan::At { offset: self.offset as i64 };
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
            fields: self.plans,
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
    assert_eq!(report.offsets, vec![0, 4, 8, 16]);
    assert_eq!(report.size, Some(24));
    assert_eq!(report.align, 8);
}

#[test]
fn effectful_policies_are_rejected_at_the_gate() {
    let main_path = write_program(
        "effectful-policy",
        r#"
data SchemaField { size: i64 [0..=4096]; align: i64 [1..=16]; number: i64; }
data Schema { fields: [SchemaField; 32]; field_count: i64 [0..=32]; }
data FieldPlan { case At(offset: i64); case Skip; }
data Plan { fields: [FieldPlan; 32]; entry_count: i64; size_fixed: i64; size_is_dynamic: bool; align: i64; }
boundary trait Console { machine write_line(text: String); machine exit_process(return_code: i32); }
data Chatty { console: Console; plans: [FieldPlan; 32]; }
machine Chatty::plan(&mut self, schema: Schema) -> Plan {
    self.console.write_line("planning...");
    Plan { fields: self.plans, entry_count: schema.field_count as i64,
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
        error.contains("not effect-free"),
        "expected a purity rejection (static gate or the dynamic backstop), got: {error}"
    );
}

#[test]
fn overlapping_plans_are_rejected_by_validation() {
    let main_path = write_program(
        "overlap-policy",
        r#"
data SchemaField { size: i64 [0..=4096]; align: i64 [1..=16]; number: i64; }
data Schema { fields: [SchemaField; 32]; field_count: i64 [0..=32]; }
data FieldPlan { case At(offset: i64); case Skip; }
data Plan { fields: [FieldPlan; 32]; entry_count: i64; size_fixed: i64; size_is_dynamic: bool; align: i64; }
data Overlapper { plans: [FieldPlan; 32]; }
machine Overlapper::plan(&mut self, schema: Schema) -> Plan {
    // ZII: every placement is At(offset: 0) -- overlapping for any schema
    // with 2+ fields.
    Plan { fields: self.plans, entry_count: schema.field_count as i64,
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
