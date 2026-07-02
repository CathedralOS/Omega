//! L3 of the LAYOUTS ladder: the zero-codegen plan pipeline, end to end. The
//! pilot policy is the brief's headline claim made real -- THE C ABI AS ~15
//! LINES OF OMEGA: an effect-free `CLayout::plan` machine (round up to the
//! field's alignment, place, track the widest alignment, round the total) is
//! evaluated at BUILD TIME against a compiler-materialized Schema, and the
//! compiler VALIDATES the plan before reporting it. A buggy policy is a
//! compile error, never unsafety -- which is also why the policy's scratch
//! arithmetic may honestly declare Wrapping: plan validation owns soundness.

use omega_compiler::{compile_to_checked, compute_layout_plan};
use std::fs;
use std::path::PathBuf;

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
data Plan {
    offsets: [i64; 32];
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
    offsets: [i64; 32];
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
        self.offsets[self.index] = self.offset as i64;
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
            offsets: self.offsets,
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
data Plan { offsets: [i64; 32]; entry_count: i64; size_fixed: i64; size_is_dynamic: bool; align: i64; }
boundary trait Console { machine write_line(text: String); machine exit_process(return_code: i32); }
data Chatty { console: Console; offsets: [i64; 32]; }
machine Chatty::plan(&mut self, schema: Schema) -> Plan {
    self.console.write_line("planning...");
    Plan { offsets: self.offsets, entry_count: schema.field_count as i64,
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
data Plan { offsets: [i64; 32]; entry_count: i64; size_fixed: i64; size_is_dynamic: bool; align: i64; }
data Overlapper { offsets: [i64; 32]; }
machine Overlapper::plan(&mut self, schema: Schema) -> Plan {
    // Every field at offset 0: overlapping for any schema with 2+ fields.
    Plan { offsets: self.offsets, entry_count: schema.field_count as i64,
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
