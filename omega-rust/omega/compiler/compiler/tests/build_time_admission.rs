use compiler::compile_to_checked;
use std::fs;
use std::path::PathBuf;

fn write_program(name: &str, source: &str) -> PathBuf {
    let directory =
        std::env::temp_dir().join(format!("omega-build-time-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&directory);
    fs::create_dir_all(&directory).expect("create temporary program directory");
    let main_path = directory.join("main.omg");
    fs::write(&main_path, source).expect("write build-time admission program");
    main_path
}

fn compile_error(name: &str, source: &str) -> String {
    let main_path = write_program(name, source);
    let diagnostics = compile_to_checked(&main_path, None)
        .expect_err("the build-time contract violation must reject compilation");
    let rendered = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let _ = fs::remove_dir_all(main_path.parent().expect("temporary program has a parent"));
    rendered
}

#[test]
fn const_evaluation_rejects_possible_suspension_independently() {
    let error = compile_error(
        "suspends",
        r#"
boundary machine wait_for_length() -> u64 suspends;

machine length() -> u64 {
    let value: u64 = suspend wait_for_length();
    transition { _ -> value }
}

data Buffer { bytes: [u8; length()]; }
data Main { }
machine Main::main(&mut self) { }
"#,
    );

    assert!(
        error.contains("machine `length` is not build-time admissible"),
        "{error}"
    );
    assert!(error.contains("may suspend"), "{error}");
    assert!(!error.contains("may block"), "{error}");
    assert!(!error.contains("service reach ["), "{error}");
}

#[test]
fn const_evaluation_rejects_possible_blocking_independently() {
    let error = compile_error(
        "blocks",
        r#"
boundary machine blocking_length() -> u64 blocks;

machine helper() -> u64 {
    let value: u64 = block blocking_length();
    transition { _ -> value }
}

machine length() -> u64 {
    let value: u64 = helper();
    transition { _ -> value }
}

data Buffer { bytes: [u8; length()]; }
data Main { }
machine Main::main(&mut self) { }
"#,
    );

    assert!(
        error.contains("machine `length` is not build-time admissible"),
        "{error}"
    );
    assert!(error.contains("may block"), "{error}");
    assert!(!error.contains("may suspend"), "{error}");
    assert!(!error.contains("service reach ["), "{error}");
}

#[test]
fn const_evaluation_rejects_a_transitive_callee_without_checked_termination() {
    let error = compile_error(
        "unterminating-callee-contract",
        r#"
machine countdown(remaining: u64) -> u64 {
    transition remaining > 0 {
        true -> countdown(remaining - 1)
        false -> 4
    }
}

machine length() -> u64 {
    countdown(3)
}

data Buffer { bytes: [u8; length()]; }
data Main { }
machine Main::main(&mut self) { }
"#,
    );

    assert!(
        error.contains("machine `length` is not build-time admissible"),
        "{error}"
    );
    assert!(
        error.contains(
            "machine call path `length -> countdown` has no ordinary checked `Terminates` guarantee"
        ),
        "{error}"
    );
    assert!(!error.contains("may suspend"), "{error}");
    assert!(!error.contains("may block"), "{error}");
    assert!(!error.contains("service reach ["), "{error}");
}

#[test]
fn const_evaluation_accepts_a_transitive_ranked_termination_proof() {
    let main_path = write_program(
        "terminating-callee-contract",
        r#"
machine countdown(remaining: u64) -> u64
terminates by remaining;
{
    transition remaining > 0 {
        true -> countdown(remaining - 1)
        false -> 4
    }
}

machine length() -> u64 {
    countdown(3)
}

data Buffer { bytes: [u8; length()]; }
data Main { }
machine Main::main(&mut self) { }
"#,
    );

    let checked = compile_to_checked(&main_path, None)
        .expect("a transitive checked termination proof should admit constant evaluation");
    let buffer = checked
        .typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Buffer")
        .expect("Buffer data definition");
    let field = checked
        .typed
        .data_members(buffer)
        .iter()
        .find_map(|member| match member {
            typed_trees::data::DataMember::Field(field) => Some(field),
            _ => None,
        })
        .expect("Buffer.bytes field");
    let typed_trees::types::TypeReferenceNode::FixedArray { length, .. } = checked
        .typed
        .type_reference_table
        .type_reference(field.type_reference)
    else {
        panic!("Buffer.bytes should remain a fixed array");
    };
    assert_eq!(length, &typed_trees::types::FixedArrayLength::Literal(4));

    let _ = fs::remove_dir_all(main_path.parent().expect("temporary program has a parent"));
}

#[test]
fn const_evaluation_rejects_an_unadmitted_linear_runtime_carrier() {
    let error = compile_error(
        "linear-runtime-carrier",
        r#"
data Ticket [linear] { code: u64; }
machine Ticket::ack(self) -> u64 { self.code }

machine length() -> u64 {
    let ticket: Ticket = Ticket { code: 4 };
    Ticket::ack(ticket)
}

data Buffer { bytes: [u8; length()]; }
data Main { }
machine Main::main(&mut self) { }
"#,
    );

    assert!(
        error.contains("machine `length` is not build-time admissible"),
        "{error}"
    );
    assert!(
        error.contains("state `entry` local `ticket` has linear runtime type `Ticket`"),
        "{error}"
    );
    assert!(error.contains("has no proof/build-admission"), "{error}");
}

#[test]
fn const_evaluation_rejects_an_undischarged_authored_precondition() {
    let error = compile_error(
        "undischarged-precondition",
        r#"
machine length() -> u64
requires true;
{
    4
}

data Buffer { bytes: [u8; length()]; }
data Main { }
machine Main::main(&mut self) { }
"#,
    );

    assert!(
        error.contains("machine `length` is not build-time admissible"),
        "{error}"
    );
    assert!(error.contains("authored `requires` premise"), "{error}");
    assert!(error.contains("no checked invocation proof"), "{error}");
}
