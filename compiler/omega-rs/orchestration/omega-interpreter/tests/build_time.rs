//! BUILD-TIME EVALUATION (L0 of the R2 layouts ladder): the compiler invokes
//! an effect-free machine through the reference interpreter with
//! compiler-built STRUCTURED arguments and reads back a structured value.
//! This pilot runs a plan-shaped machine -- a policy taking a schema-like
//! struct and returning a plan-like struct -- the exact call shape the Layout
//! machinery makes (programmable_layouts.md).

use omega_compiler::compile_to_checked;
use omega_interpreter::{BuildTimeValue, evaluate_build_time_machine};
use std::fs;
use std::path::PathBuf;

fn write_program(name: &str, source: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "omega-build-time-{name}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create temp program dir");
    let main_path = dir.join("main.omg");
    fs::write(&main_path, source).expect("write build-time program");
    main_path
}

#[test]
fn plan_shaped_machine_evaluates_with_structured_argument() {
    // A two-field "schema" in, a "plan" (offsets + size) out: the C-layout
    // rule for two fields, written as an ordinary effect-free machine. Ranges
    // keep the arithmetic Exact (sizes are verifiably bounded).
    let main_path = write_program(
        "plan-pilot",
        r#"
data Schema {
    a_size: i64 [0..=4096];
    b_size: i64 [0..=4096];
}
data Plan {
    offset_a: i64;
    offset_b: i64;
    size: i64;
}
data Planner { }
machine Planner::plan(&self, schema: Schema) -> Plan {
    Plan {
        offset_a: 0,
        offset_b: schema.a_size,
        size: schema.a_size + schema.b_size,
    }
}
data Main { }
machine Main::main(&mut self) { }
"#,
    );

    let checked = compile_to_checked(&main_path, None).expect("pilot program should compile");
    let schema = BuildTimeValue::Struct {
        type_name: "Schema".to_owned(),
        fields: vec![
            ("a_size".to_owned(), BuildTimeValue::Int(8)),
            ("b_size".to_owned(), BuildTimeValue::Int(24)),
        ],
    };

    let plan = evaluate_build_time_machine(&checked.typed, "Planner::plan", vec![schema])
        .expect("plan() should evaluate at build time");

    let BuildTimeValue::Struct { type_name, fields } = plan else {
        panic!("expected a struct plan, got {plan:?}");
    };
    assert_eq!(type_name, "Plan");
    let field = |name: &str| {
        fields
            .iter()
            .find(|(field, _)| field == name)
            .map(|(_, value)| value.clone())
            .unwrap_or_else(|| panic!("plan should carry `{name}`"))
    };
    assert_eq!(field("offset_a"), BuildTimeValue::Int(0));
    assert_eq!(field("offset_b"), BuildTimeValue::Int(8));
    assert_eq!(field("size"), BuildTimeValue::Int(32));
}

#[test]
fn argument_count_mismatch_reports_clearly() {
    let main_path = write_program(
        "plan-arity",
        r#"
data Planner { }
machine Planner::plan(&self, size: i64 [0..=4096]) -> i64 {
    size
}
data Main { }
machine Main::main(&mut self) { }
"#,
    );

    let checked = compile_to_checked(&main_path, None).expect("arity program should compile");
    let error = evaluate_build_time_machine(&checked.typed, "Planner::plan", Vec::new())
        .expect_err("missing argument should be a clear error");
    assert!(
        error.contains("takes 1 argument"),
        "expected the arity message, got: {error}"
    );
}
