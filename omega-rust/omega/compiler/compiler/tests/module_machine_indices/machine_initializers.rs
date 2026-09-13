use super::{Sources, compile, root_inputs};
use terminal_interpreter::{TerminalExecutionResult, interpret_terminal_artifact};

fn assert_source_free_result(checked: compiler::CheckedCompilation, machine: &str, expected: u64) {
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, machine)
        .produce_artifact()
        .expect("machine-computed constant reaches Terminal");
    drop(checked);
    assert_eq!(
        interpret_terminal_artifact(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &proof_admission::AdmissionProfile::default(),
            &[],
        )
        .expect("machine-computed constant executes without checked source"),
        TerminalExecutionResult::Scalar(super::array_construction::integer(
            u128::from(expected),
            64
        )),
    );
}

#[test]
fn machine_constant_customer_publishes_and_executes_without_source() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("main.omg"),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../tests/omega/pass/modules/machine_constant_initializers/main.omg"
        )),
    );
    assert_source_free_result(compile(&root, root_inputs(&root)), "read", 7);
}

#[test]
fn machine_constant_closure_preserves_module_selection_and_index_identity() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("settings.omg"),
        "module settings;
        machine size() -> u64 { BASE }
        machine identity(value: u64) -> u64 { value }
        pub const SIZE: u64 = identity(size()) * 2;
        const BASE: u64 = 7 / 2 * 2;",
    );
    Sources::write(
        root.join("main.omg"),
        &format!(
            "use settings; {}
        machine size() -> u64 {{ 99 }}
        const BASE: u64 = 99;
        machine read() -> u64 {{ settings::SIZE }} {} {}",
            super::BUFFER,
            super::keep("keep", "settings::SIZE"),
            super::keep("oracle", "14")
        ),
    );
    let checked = compile(&root, root_inputs(&root));
    super::assert_same_machine_types(&checked, "keep", "oracle");
    assert_source_free_result(checked, "read", 14);
}

#[test]
fn machine_calls_compose_with_nominal_projection_and_boolean_selection() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("main.omg"),
        "data Config [copy] { size: u64; enabled: bool; }
        machine size() -> u64 { 7 }
        machine truth(value: bool) -> bool { value }
        const CONFIG: Config = Config { size: size() * 2, enabled: truth(size() == 7) };
        const SELECTED: u64 = match truth(true) { true -> size(), false -> size() * 2 };
        machine read() -> u64 { CONFIG.size + SELECTED }",
    );
    assert_source_free_result(compile(&root, root_inputs(&root)), "read", 21);
}

#[test]
fn machine_constant_record_retains_payloadless_case_siblings() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("main.omg"),
        "data Mode [copy] { case On; case Off; }
        data Config [copy] { mode: Mode; count: u64; }
        machine size() -> u64 { 7 }
        const CONFIG: Config = Config { mode: Mode::On, count: size() };
        machine read() -> u64 { 7 }",
    );
    assert_source_free_result(compile(&root, root_inputs(&root)), "read", 7);
}

#[test]
fn invalid_unused_machine_constant_initializers_reject() {
    let tree = Sources::new();
    let root = tree.package("root");
    for source in [
        "machine narrow() -> u8 { 7 } const SIZE: u64 = narrow();",
        "machine wide(value: u64) -> u64 { value } const SIZE: u64 = wide(7u8);",
        "machine size() -> u64 { SIZE } const SIZE: u64 = size();",
        "machine size() -> u64 { size() } const SIZE: u64 = size();",
        "machine divide(value: u64) -> u64 { 10 / value } const SIZE: u64 = divide(0);",
    ] {
        Sources::write(root.join("main.omg"), source);
        let result = compiler::compile_to_checked(compiler::CheckedCompileRequest {
            package_inputs: Some(root_inputs(&root)),
            ..compiler::CheckedCompileRequest::new(&root.join("main.omg"), None)
        });
        assert!(
            result.is_err(),
            "invalid unused initializer was accepted: {source}"
        );
    }
}
