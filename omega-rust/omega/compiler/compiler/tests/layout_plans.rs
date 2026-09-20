//! L3 of the LAYOUTS ladder: the zero-codegen plan pipeline, end to end. The
//! pilot policy is the brief's headline claim made real -- THE C ABI AS ~15
//! LINES OF OMEGA: an effect-free `CLayout::plan` machine (round up to the
//! field's alignment, place, track the widest alignment, round the total) is
//! evaluated at BUILD TIME against a compiler-materialized Schema, and the
//! compiler VALIDATES the plan before reporting it. A buggy policy is a
//! compile error, never unsafety -- which is also why the policy's scratch
//! arithmetic may honestly declare Wrapping: plan validation owns soundness.
//!
//! Fixtures shared by the layout plan tests: written programs.

#[path = "layout_plans/callback_slots.rs"]
mod callback_slots;
#[path = "layout_plans/field_reflection_and_materialization.rs"]
mod field_reflection_and_materialization;
#[path = "fixture_rosters/layout_plans.rs"]
mod fixture_roster;
#[path = "layout_plans/interrupt_descriptor_tables.rs"]
mod interrupt_descriptor_tables;
#[path = "../../../../../tests/native-differential/tests/common/native_function.rs"]
#[allow(dead_code)]
mod native_function;
#[path = "layout_plans/plan_validation_and_bit_placements.rs"]
mod plan_validation_and_bit_placements;
#[path = "layout_plans/writer_lowering.rs"]
mod writer_lowering;

use build_declarations::{BuildDeclaration, extract_build_declaration};
use calling_conventions::MachineRegister;
use layout_plans::{PostHandoffWriterPlan, RelocationTarget};
use package_compilation::{
    PackageCompilationInputs, PackageDependencyBinding, PackageSourceBinding,
};
use semantic_vocabulary::PackageKeyIdentity;
use std::fs;
use std::path::{Path, PathBuf};
use target::NativeTarget;

// Reuse the host linker/executor without modifying the shared differential owner.
#[allow(dead_code)]
fn write_program(name: &str, source: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("omega-layout-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create temp program dir");
    let main_path = dir.join("main.omg");
    fs::write(&main_path, source).expect("write layout program");
    main_path
}

fn package_inputs_for_source(source: &Path, digest_byte: u8) -> PackageCompilationInputs {
    let package = PackageKeyIdentity::from_digest([digest_byte; 32])
        .expect("test package identity should be nonzero");
    PackageCompilationInputs::new_package(
        package,
        vec![PackageSourceBinding::new(
            package,
            "layout-plan-canary",
            source.parent().expect("source parent").to_owned(),
        )],
        Vec::new(),
    )
    .expect("test package inputs should validate")
}

/// The three hosted layout canaries declare this ordinary std dependency.
/// Supply the explicit graph just as the repository canary harness does; the
/// checked/layout-only tests do not issue provider acceptance or native code.
fn package_inputs_with_standard_library(source: &Path) -> PackageCompilationInputs {
    let root = source.parent().expect("canary project root");
    let declaration = extract_build_declaration(root).expect("authored canary declaration");
    let role = declaration.kind();
    let name = match declaration {
        BuildDeclaration::Application(application) => application.name,
        BuildDeclaration::Package(package) => package.name,
        BuildDeclaration::Workspace(_) => panic!("a layout canary is not a workspace root"),
    };
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("repository root");
    let package = PackageKeyIdentity::from_digest([1; 32]).expect("canary identity");
    let standard_library = PackageKeyIdentity::from_digest([2; 32]).expect("std identity");
    PackageCompilationInputs::new(
        package,
        role,
        vec![
            PackageSourceBinding::new(package, name.into_string(), root.to_owned()),
            PackageSourceBinding::new(
                standard_library,
                "omega-language-std",
                repository.join("source/library/std"),
            ),
        ],
        vec![PackageDependencyBinding::new(
            package,
            "omega_language_std",
            standard_library,
        )],
    )
    .expect("ordinary std dependency graph")
}

/// The vocabulary (mirrors source/library/core/layout.omg) + the CLayout
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
    // The returned entries are owned state data; the borrowed policy retains
    // only scalar scratch, so completion never moves out of its receiver.
    let entries: [FieldEntry; 64];
    self.index = 0;
    self.offset = 0;
    self.widest = 1;
    transition { _ -> place_loop(schema, entries, fuel) }

    state place_loop(&mut self, schema: Schema, entries: [FieldEntry; 64], fuel: u64 [1..=256]) {
        transition fuel > 1 && self.index < 32 && self.index < schema.field_count {
            true -> read_field(schema, entries, fuel - 1)
            _ -> done(schema, entries)
        }
    }
    state read_field(&mut self, schema: Schema, entries: [FieldEntry; 64], fuel: u64 [1..=256]) {
        self.fsize = schema.fields[self.index].size;
        self.falign = schema.fields[self.index].align;
        // C rule: round up to the field's alignment, place, advance.
        self.pad = (self.falign - self.offset % self.falign) % self.falign;
        self.offset = self.offset + self.pad;
        transition fuel > 1 && self.index < 32 {
            true -> place_field(schema, entries, fuel - 1)
            _ -> done(schema, entries)
        }
    }
    state place_field(&mut self, schema: Schema, entries: [FieldEntry; 64], fuel: u64 [1..=256]) {
        let mut updated: [FieldEntry; 64] = entries;
        updated[self.index] = FieldEntry {
            key: schema.fields[self.index].key,
            placement: FieldPlan::At { offset: self.offset as u64 },
        };
        self.offset = self.offset + self.fsize;
        transition fuel > 1 {
            true -> choose_widen(schema, updated, fuel - 1)
            _ -> done(schema, updated)
        }
    }
    state choose_widen(&mut self, schema: Schema, entries: [FieldEntry; 64], fuel: u64 [1..=256]) {
        transition {
            fuel > 1 && self.widest < self.falign -> widen(schema, entries, fuel - 1)
            fuel > 1 -> advance(schema, entries, fuel - 1)
            _ -> done(schema, entries)
        }
    }
    state widen(&mut self, schema: Schema, entries: [FieldEntry; 64], fuel: u64 [1..=256]) {
        self.widest = self.falign;
        transition fuel > 1 {
            true -> advance(schema, entries, fuel - 1)
            _ -> done(schema, entries)
        }
    }
    state advance(&mut self, schema: Schema, entries: [FieldEntry; 64], fuel: u64 [1..=256]) {
        self.index = self.index + 1;
        transition fuel > 1 {
            true -> place_loop(schema, entries, fuel - 1)
            _ -> done(schema, entries)
        }
    }
    state done(&mut self, schema: Schema, entries: [FieldEntry; 64]) -> Plan {
        // Round the total size up to the struct alignment (the C tail rule).
        self.pad = (self.widest - self.offset % self.widest) % self.widest;
        Plan {
            entries: entries,
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

/// Both Linux ISAs realize the same derived post-handoff writer. The
/// normalized fragment identity is target-independent — physical lowering may
/// choose instruction bytes and a context register but cannot change which
/// semantic slot the write addresses — while the emitted bytes and machine
/// footprint belong to the selected architecture. Exact replay validation
/// proves each lowered fragment still carries the checked write geometry.
///
/// On a matching Linux host, native execution is compared with the Rust
/// reference image so target-dependent instruction selection must preserve the
/// exact semantic slots.
fn lower_writer_on_both_linux_isas(
    writer: &PostHandoffWriterPlan,
    fill: u8,
    expected_image: &[u8],
    resolve: impl Fn(RelocationTarget) -> u64,
) {
    let x86 = program_entry_plan::lower_post_handoff_writer_fragment(
        NativeTarget::linux_x64(),
        MachineRegister::X86Rdi,
        writer,
    )
    .expect("the symbolic writer lowers to linux_x86_64 code");
    let arm = program_entry_plan::lower_post_handoff_writer_fragment(
        NativeTarget::linux_arm64(),
        MachineRegister::Aarch64X(0),
        writer,
    )
    .expect("the symbolic writer lowers to linux_arm64 code");
    program_entry_plan::validate_lowered_post_handoff_writer(&x86)
        .expect("the linux_x86_64 writer fragment replays exactly");
    program_entry_plan::validate_lowered_post_handoff_writer(&arm)
        .expect("the linux_arm64 writer fragment replays exactly");
    assert_eq!(
        x86.fragment().normalized_plan_report_fingerprint(),
        arm.fragment().normalized_plan_report_fingerprint(),
        "both Linux ISAs realize the same normalized writer fragment"
    );
    assert_ne!(
        x86.fragment().bytes(),
        arm.fragment().bytes(),
        "each ISA emits its own target-dependent writer bytes"
    );
    assert_eq!(x86.invocation(), arm.invocation());
    let invocation = x86.invocation();
    let source_values = invocation
        .sources()
        .iter()
        .map(|slot| resolve(slot.target))
        .collect::<Vec<_>>();
    invocation
        .validate_source_values(&source_values)
        .expect("resolved source values satisfy the writer invocation");
    assert_eq!(expected_image.len(), writer.byte_len);

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    let bytes = {
        let mut bytes = x86.fragment().bytes().to_vec();
        bytes.push(0xc3);
        bytes
    };
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    let bytes = {
        let mut bytes = arm.fragment().bytes().to_vec();
        bytes.extend_from_slice(&0xd65f03c0_u32.to_le_bytes());
        bytes
    };
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    let bytes = {
        let mac = program_entry_plan::lower_post_handoff_writer_fragment(
            NativeTarget::macos_arm64(),
            MachineRegister::Aarch64X(0),
            writer,
        )
        .expect("the symbolic writer lowers to macos_arm64 code");
        program_entry_plan::validate_lowered_post_handoff_writer(&mac)
            .expect("the macos_arm64 writer fragment replays exactly");
        assert_eq!(mac.invocation(), invocation);
        assert_eq!(
            mac.fragment().normalized_plan_report_fingerprint(),
            arm.fragment().normalized_plan_report_fingerprint(),
        );
        let mut bytes = mac.fragment().bytes().to_vec();
        bytes.extend_from_slice(&0xd65f03c0_u32.to_le_bytes());
        bytes
    };
    #[cfg(not(any(
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "aarch64")
    )))]
    {
        let _ = fill;
        eprintln!("skip: native writer execution needs Linux x86-64/aarch64 or macOS aarch64");
        return;
    }

    #[cfg(any(
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "aarch64")
    ))]
    {
        let expected = expected_image
            .iter()
            .map(|byte| format!("0x{byte:02x}"))
            .collect::<Vec<_>>()
            .join(", ");
        let source_context = source_values
            .iter()
            .enumerate()
            .map(|(index, value)| format!("    context[{}] = 0x{value:016x}ULL;", index + 1))
            .collect::<Vec<_>>()
            .join("\n");
        let driver = format!(
            r#"#include <stdint.h>
#include <string.h>

extern void omega_entry(uint64_t *context);

static const uint8_t expected[] = {{{expected}}};

int main(void) {{
    struct {{
        uint64_t before;
        uint8_t destination[{byte_len}];
        uint64_t after;
    }} image;
    uint64_t context[1 + {source_slot_count}];
    const uint64_t guard = UINT64_C(0x5a5a5a5a5a5a5a5a);

    image.before = guard;
    image.after = guard;
    memset(image.destination, {fill}, {byte_len});
    context[0] = (uint64_t)(uintptr_t)image.destination;
{source_context}
    omega_entry(context);
    omega_entry(context);
    return memcmp(image.destination, expected, {byte_len})
        ? 1
        : (image.before != guard || image.after != guard) ? 2 : 0;
}}
"#,
            byte_len = writer.byte_len,
            source_slot_count = source_values.len(),
        );
        native_function::assert_c_text(&bytes, 0, &driver);
    }
}
