//! MINT ARC RUNG 2a: every numbered schema gets a DERIVED WIRE PLAN --
//! `Varint(tag)` for scalars, `LengthPrefixed(tag)` for text/nested/repeated
//! -- recorded arena+span on TypedTrees and consumed (load-bearing tags) by
//! the wire codec selection. Byte identity is pinned by the wire run
//! canaries; this test pins the PLAN ITSELF.

use omega_compiler::compile_to_checked;
use omega_typed_trees::wire::WirePlacement;
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
data Packet {
    3: label: String;
    1: seed: u64;
    2: flag: bool;
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
    // (scalar varint), 3=label (String, length-prefixed).
    assert_eq!(
        plan,
        [
            WirePlacement::Varint { tag: 1 },
            WirePlacement::Varint { tag: 2 },
            WirePlacement::LengthPrefixed { tag: 3 },
        ]
    );
}
