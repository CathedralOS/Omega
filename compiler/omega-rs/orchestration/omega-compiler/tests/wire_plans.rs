//! MINT ARC RUNG 2a: every numbered schema gets a DERIVED WIRE PLAN --
//! `Varint(tag)` for scalars, `LengthPrefixed(tag)` for text/nested/repeated
//! -- recorded arena+span on TypedTrees and consumed (load-bearing tags) by
//! the wire codec selection. Byte identity is pinned by the wire run
//! canaries; this test pins the PLAN ITSELF.

use omega_compiler::compile_to_checked;
use psi_typed_trees::wire::WirePlacement;
use std::fs;
use std::path::PathBuf;

fn write_program(name: &str, source: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("omega-wire-plans-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create temp program dir");
    let main_path = dir.join("main.omg");
    fs::write(&main_path, source).expect("write temp program");
    main_path
}

#[test]
fn numbered_schemas_get_tag_ordered_placement_plans() {
    let main_path = write_program(
        "mixed",
        r#"
use omega::language::core::fixed_vec;

data Packet {
    #3 label: &[u8];
    #1 seed: u64;
    #2 flag: bool;
    #4 samples: FixedVec<i32, 4>;
    #5 borrowed: &[i32];
}
data Main { }
machine Main::main(&mut self) { }
"#,
    );
    let checked = compile_to_checked(&main_path, None).expect("program should compile");
    let schema = checked
        .typed
        .wire_schemas()
        .iter()
        .find(|schema| schema.name.as_str() == "Packet")
        .expect("Packet schema");
    let plan = checked
        .typed
        .wire_schema_plan(schema.symbol)
        .expect("Packet should carry a derived wire plan");
    // Placements are TAG-ORDERED (the codec emits in field-number order),
    // regardless of declaration order: 1=seed (scalar varint), 2=flag
    // (scalar varint), 3=label (borrowed bytes, length-prefixed), 4=samples
    // (bounded repeated FixedVec, length-prefixed).
    assert_eq!(
        plan,
        [
            WirePlacement::Varint { tag: 1 },
            WirePlacement::Varint { tag: 2 },
            WirePlacement::LengthPrefixed { tag: 3 },
            WirePlacement::LengthPrefixed { tag: 4 },
            WirePlacement::LengthPrefixed { tag: 5 },
        ]
    );
    let obligations = checked
        .typed
        .wire_schema_encode_obligations(schema.symbol)
        .expect("Packet plan should expose encode obligations");
    assert_eq!(obligations.len(), 1);
    assert_eq!(obligations[0].field_number, 5);
    assert_eq!(obligations[0].element.byte_size, 4);
    assert!(obligations[0].element.zigzag);
}

#[test]
fn full_width_unsigned_policy_tags_are_not_reinterpreted_as_signed() {
    let main_path = write_program(
        "full-width-tag",
        r#"
data Packet { #1 value: u8; }

data FieldKind { case Scalar; case Text; case Nested; case Repeated; }
data SchemaField { size: u64 [0..=4096]; align: u64 [1..=16]; number: i64; kind: FieldKind; }
data Schema { fields: [SchemaField; 32]; field_count: u64 [0..=32]; }
data FieldPlan { case Varint(tag: u64); case LengthPrefixed(tag: u64); }
data Plan { fields: [FieldPlan; 32]; entry_count: u64; size_fixed: u64; size_is_dynamic: bool; align: u64; }

data CompactBinary { fields: [FieldPlan; 32]; }
machine CompactBinary::plan(&mut self, schema: Schema) -> Plan {
    self.fields[0] = FieldPlan::Varint { tag: 18446744073709551615 };
    Plan {
        fields: self.fields,
        entry_count: schema.field_count,
        size_fixed: 0,
        size_is_dynamic: true,
        align: 1,
    }
}

data Main { }
machine Main::main(&mut self) { }
"#,
    );
    let diagnostics = compile_to_checked(&main_path, None)
        .expect_err("a full-width authored tag must disagree with schema tag 1");
    let rendered = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    assert!(
        rendered.contains("tag: 18446744073709551615"),
        "unexpected diagnostic: {rendered}"
    );
}
