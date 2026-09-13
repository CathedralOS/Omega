use super::{Sources, compile, root_inputs};
use terminal_interpreter::{TerminalExecutionResult, interpret_terminal_artifact};

#[test]
fn computed_nominal_customer_publishes_and_executes_without_source() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("main.omg"),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../tests/omega/pass/modules/computed_nominal_constants/main.omg"
        )),
    );
    let checked = compile(&root, root_inputs(&root));
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "read")
        .produce_artifact()
        .expect("computed record field reaches Terminal");
    drop(checked);
    let result = interpret_terminal_artifact(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &proof_admission::AdmissionProfile::default(),
        &[],
    )
    .expect("source-free nominal constant consumer");
    assert_eq!(
        result,
        TerminalExecutionResult::Scalar(super::array_construction::integer(14, 64))
    );
}

#[test]
fn computed_nominal_indices_keep_exact_module_carriers_and_private_dependencies() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("settings.omg"),
        "module settings;
        pub data Config [copy] { count: u64; enabled: bool; }
        const SIZE: u64 = 7 / 2 * 2;
        pub const CONFIG: Config = Config { count: SIZE * 2, enabled: SIZE == 7 };
        pub const LITERAL: Config = Config { enabled: true, count: 14 };",
    );
    Sources::write(
        root.join("main.omg"),
        "use settings;
        data Config [copy] { wrong: i32; }
        const SIZE: u64 = 99;
        data Pick<const Selected: settings::Config> { marker: u8; }
        machine keep(value: Pick<settings::CONFIG>) -> Pick<settings::CONFIG> { let local: Pick<settings::CONFIG> = value; local }
        machine oracle(value: Pick<settings::LITERAL>) -> Pick<settings::LITERAL> { let local: Pick<settings::LITERAL> = value; local }
        machine read() -> u64 { settings::CONFIG.count }",
    );
    let checked = compile(&root, root_inputs(&root));
    super::assert_same_machine_types(&checked, "keep", "oracle");
    assert!(!super::selections(&checked, "settings::CONFIG", super::identity(1)).is_empty());
    let dependencies = super::selections(&checked, "settings::SIZE", super::identity(1));
    assert!(!dependencies.is_empty());
    assert!(dependencies.iter().all(|selection| selection.exposure() ==
        language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PrivateImplementation));
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "read")
        .produce_artifact()
        .expect("module-owned record projection reaches Terminal");
    drop(checked);
    assert_eq!(
        interpret_terminal_artifact(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &proof_admission::AdmissionProfile::default(),
            &[],
        )
        .expect("source-free module-owned nominal constant consumer"),
        TerminalExecutionResult::Scalar(super::array_construction::integer(14, 64))
    );
}
