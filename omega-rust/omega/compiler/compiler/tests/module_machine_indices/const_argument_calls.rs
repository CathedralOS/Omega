//! Const arguments compose ordinary calls under the expression schedule.

use super::{Sources, compile, root_inputs};
use terminal_interpreter::{TerminalExecutionResult, interpret_terminal_artifact};

fn assert_integer_argument(expression: &str, expected: u64) {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("main.omg"),
        &format!(
            "machine size(value: u64) -> u64 {{ value }}
         machine seven() -> u64 {{ 7 }}
         machine ready() -> bool {{ true }}
         machine identity<T>(value: T) -> T {{ value }}
         machine unavailable() -> u64 requires false {{ 99 }}
         domain<const N: u64> u64::Indexed<N>;
         machine consume(value: u64 in Indexed<{expected}>) -> u64 {{ value as u64 }}
         machine read() -> u64 {{
             let value: u64 in Indexed<{expression}> = {expected} as u64 in Indexed<{expected}>;
             consume(value)
         }}"
        ),
    );
    let checked = compile(&root, root_inputs(&root));
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "read")
        .produce_artifact()
        .expect("computed argument reaches Terminal");
    drop(checked);
    drop(tree);
    assert!(!root.exists());
    assert_eq!(
        interpret_terminal_artifact(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &proof_admission::AdmissionProfile::default(),
            &[]
        )
        .expect("source-free argument result"),
        TerminalExecutionResult::Scalar(super::array_construction::integer(
            u128::from(expected),
            64
        )),
    );
}

#[test]
fn arithmetic_const_arguments_compose_closed_calls() {
    assert_integer_argument("(size(4) + 2)", 6);
    assert_integer_argument("(seven() + size(4))", 11);
}

#[test]
fn selective_const_arguments_compose_subject_and_arm_calls() {
    assert_integer_argument("(match ready() { true -> size(7), false -> size(9) })", 7);
    assert_integer_argument(
        "(match ready() { true -> seven(), false -> unavailable() })",
        7,
    );
}

#[test]
fn const_arguments_admit_closed_generic_helpers() {
    assert_integer_argument("identity<u64>(7)", 7);
}

fn rejection(source: &str) -> String {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(root.join("main.omg"), source);
    compiler::compile_to_checked(compiler::CheckedCompileRequest::new(
        &root.join("main.omg"),
        None,
    ))
    .expect_err("invalid const argument rejects")
    .iter()
    .map(|diagnostic| diagnostic.message.as_str())
    .collect::<Vec<_>>()
    .join("\n")
}

#[test]
fn const_argument_calls_do_not_capture_shadowed_runtime_operands() {
    for binding in [
        "machine read(VALUE: u64) -> u64 {",
        "machine read() -> u64 { let VALUE: u64 = 8;",
    ] {
        let error = rejection(&format!(
            "const VALUE: u64 = 7;
             machine size(value: u64) -> u64 {{ value }}
             data Indexed<const N: u64> {{ value: u64; }}
             {binding} let value: Indexed<size(VALUE)> = Indexed {{ value: 0 }}; 0 }}"
        ));
        assert!(
            error.contains("runtime") || error.contains("original lexical"),
            "{error}"
        );
    }
}

#[test]
fn skipped_call_arguments_still_require_compatible_types() {
    let error = rejection(
        "machine size(value: u64) -> u64 { value }
         data Indexed<const N: u64> { value: u64; }
         machine read() -> u64 {
             let value: Indexed<(match true { true -> 7, false -> size(true) })> = Indexed { value: 0 };
             0
         }"
    );
    assert!(
        error.contains("differs from its destination carrier"),
        "{error}"
    );
}

#[test]
fn demanded_calls_cannot_bypass_false_preconditions() {
    let error = rejection(
        "machine unavailable() -> u64 requires false { 99 }
         data Indexed<const N: u64> { value: u64; }
         machine read() -> u64 {
             let value: Indexed<(match true { true -> unavailable(), false -> 7 })> = Indexed { value: 0 };
             0
         }"
    );
    assert!(
        error.contains("requires") || error.contains("premise"),
        "{error}"
    );
}

#[test]
fn record_const_argument_composition_retains_exact_index_identity() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("main.omg"),
        "machine size(value: u64) -> u64 { value }
         data Indexed<const N: u64> { value: u64; }
         machine count<const N: u64>(value: Indexed<N>) -> u64 { N }
         machine read() -> u64 {
             let value: Indexed<(size(4) + 2)> = Indexed { value: 0 };
             count(value)
         }",
    );
    let _checked = compile(&root, root_inputs(&root));
}

#[test]
fn const_argument_helpers_retain_unrelated_indexed_locals() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("main.omg"),
        "machine size(value: u64) -> u64 { value }
         data Indexed<const N: u64> { value: u64; }
         machine seven() -> u64 {
             let stored: Indexed<size(4)> = Indexed { value: 7 }; stored.value
         }
         data Holder { value: Indexed<seven()>; }
         machine keep(value: Indexed<seven()>) -> Indexed<7> { value }",
    );
    let _checked = compile(&root, root_inputs(&root));
}

#[test]
fn const_argument_calls_keep_module_selected_helpers_and_constants() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("settings.omg"),
        "module settings; pub const VALUE: u64 = 7;
         pub machine size(value: u64) -> u64 { value }",
    );
    Sources::write(
        root.join("main.omg"),
        "use settings; const VALUE: u64 = 9;
         machine size(value: u64) -> u64 { 11 }
         domain<const N: u64> u64::Index<N>;
         machine consume(value: u64 in Index<7>) -> u64 { value as u64 }
         machine read() -> u64 {
             let value: u64 in Index<settings::size(settings::VALUE)> = 7 as u64 in Index<7>;
             consume(value)
         }",
    );
    let checked = compile(&root, root_inputs(&root));
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "read")
        .produce_artifact()
        .expect("module-selected index reaches Terminal");
    drop(checked);
    drop(tree);
    assert_eq!(
        interpret_terminal_artifact(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &proof_admission::AdmissionProfile::default(),
            &[]
        )
        .unwrap(),
        TerminalExecutionResult::Scalar(super::array_construction::integer(7, 64))
    );
}

#[test]
fn short_circuit_calls_do_not_discharge_skipped_invocations() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("main.omg"),
        "machine unavailable() -> bool requires false { false }
         domain<const B: bool> u64::Gate<B>;
         machine consume(value: u64 in Gate<true>) -> u64 { value as u64 }
         machine read() -> u64 {
             let value: u64 in Gate<(!false || unavailable())> = 7 as u64 in Gate<true>;
             consume(value)
         }",
    );
    let checked = compile(&root, root_inputs(&root));
    let _artifact = terminal_production::TerminalProductionRequest::new(&checked, "read")
        .produce_artifact()
        .expect("skipped call does not become an execution requirement");
}
