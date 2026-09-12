//! Empty scalar contracts must agree with authored clauses and parameter ranges.

use checked_trees::{CheckedTrees, ClosedScalarValueContractPlan};
use checked_trees_to_lowered_psi::LoweringError;
use proof_admission::AdmissionProfile;
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
use terminal_codec::CanonicalTerminalArtifact;
use terminal_interpreter::{
    TerminalExecutionResult, TerminalScalarValue, interpret_terminal_artifact,
};

fn checked(source: &str) -> CheckedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize source");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse source");
    let resolved =
        syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).expect("resolve source");
    let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type source");
    typed_trees_to_checked_trees::lower_typed_trees(typed)
        .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"))
}

fn publish(checked: &CheckedTrees) -> (CanonicalTerminalArtifact, terminal_psi::TerminalModule) {
    let artifact = terminal_production::produce_terminal_artifact(checked, "enter")
        .expect("unmodified source must publish before custody mutations");
    let module =
        terminal_codec::decode_module(artifact.semantic_bytes()).expect("reload semantics");
    let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).expect("reload proof");
    terminal_verifier::verify_module(&module, &proof, &AdmissionProfile::default())
        .expect("independently verify the reloaded artifact");
    (artifact, module)
}

fn execute_identity(artifact: &CanonicalTerminalArtifact) {
    let input = TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
        value: IntegerValue::Unsigned(7),
    };
    assert_eq!(
        interpret_terminal_artifact(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &AdmissionProfile::default(),
            &[input],
        )
        .expect("execute source identity from the artifact"),
        TerminalExecutionResult::Scalar(input),
    );
}

fn reject_erased_contract(original: &CheckedTrees, owner: &str, expected_message: &str) {
    let mut changed = original.clone();
    let symbol = changed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == owner)
        .expect("authored contract owner")
        .symbol;
    let contract = changed
        .facts
        .contract_plans
        .machines
        .iter_mut()
        .find(|contract| contract.machine == symbol)
        .expect("retained scalar contract");
    assert!(
        !contract.closed_scalar_values.requires().is_empty()
            || !contract.closed_scalar_values.ensures().is_empty(),
        "mutation must delete actual checked contract rows"
    );
    contract.closed_scalar_values = ClosedScalarValueContractPlan::default();
    assert_eq!(
        changed.typed, original.typed,
        "authored custody is unchanged"
    );
    match checked_trees_to_lowered_psi::lower_machine(&changed, "enter") {
        Err(LoweringError::Unsupported(message)) => assert_eq!(message, expected_message),
        Err(error) => panic!("unexpected rejection for {owner}: {error:#?}"),
        Ok(_) => panic!("accepted erased scalar contract for {owner}"),
    }
    assert!(
        terminal_production::produce_terminal_artifact(&changed, "enter").is_err(),
        "erased scalar contract must not publish"
    );
}

#[test]
fn genuine_empty_contracts_publish_on_scalar_roots_and_transitive_callees() {
    let source = r#"
        machine identity(input: u64) -> u64 { input }
        machine enter(input: u64) -> u64 {
            let answer: u64 = identity(input);
            answer
        }
    "#;
    let (artifact, module) = publish(&checked(source));
    assert_eq!(module.machines.len(), 2, "retain the actual scalar callee");
    for machine in &module.machines {
        assert!(machine.contract.requires.is_empty());
        assert!(machine.contract.ensures.is_empty());
        assert!(machine.contract.crash_routes.is_empty());
    }
    execute_identity(&artifact);
}

#[test]
fn empty_normal_contract_keeps_a_published_crash_route() {
    let source = "machine enter() -> u64\ncrashes Abort\n{ crash Abort; }";
    let (_, module) = publish(&checked(source));
    assert_eq!(module.machines.len(), 1);
    let contract = &module.machines[0].contract;
    assert!(contract.requires.is_empty());
    assert!(contract.ensures.is_empty());
    assert_eq!(contract.crash_routes.len(), 1);
}

#[test]
fn empty_checked_contract_cannot_erase_authored_normal_clauses() {
    for (clauses, requires, ensures) in [
        ("requires input <= 11u64", 1, 0),
        ("ensures result == input", 0, 1),
        ("requires input <= 11u64\nensures result == input", 1, 1),
    ] {
        let source = format!("machine enter(input: u64) -> u64\n{clauses}\n{{ input }}");
        let original = checked(&source);
        let (artifact, module) = publish(&original);
        assert_eq!(module.machines.len(), 1);
        assert_eq!(module.machines[0].contract.requires.len(), requires);
        assert_eq!(module.machines[0].contract.ensures.len(), ensures);
        execute_identity(&artifact);
        reject_erased_contract(
            &original,
            "enter",
            "empty scalar contract would erase an authored normal clause",
        );
    }
}

#[test]
fn selected_callback_identity_preserves_its_normal_contract() {
    for clauses in ["", "requires input <= 11u64\nensures result == input"] {
        let original = checked(&format!(
            "data Callback {{}}\nmachine Callback::identity(input: u64) -> u64\n{clauses}\n{{ input }}"
        ));
        let graph = &original.facts.flow.terminal_scalar_graphs.machines[0];
        let lowered = checked_trees_to_lowered_psi::lower_bounded_callback_identity_machine(
            &original,
            graph.machine,
            graph.states[0].state,
        )
        .expect("selected identity body")
        .terminal;
        let contract = &lowered.semantic_module.machines[0].contract;
        let expected_count = usize::from(!clauses.is_empty());
        assert_eq!(contract.requires.len(), expected_count);
        assert_eq!(contract.ensures.len(), expected_count);
        let semantic = terminal_codec::encode_module(&lowered.semantic_module).unwrap();
        let proof = terminal_codec::encode_proof_bundle(&lowered.proof_bundle).unwrap();
        let input = TerminalScalarValue::Integer {
            scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
            value: IntegerValue::Unsigned(7),
        };
        assert_eq!(
            interpret_terminal_artifact(&semantic, &proof, &AdmissionProfile::default(), &[input])
                .unwrap(),
            TerminalExecutionResult::Scalar(input)
        );
    }
}

#[test]
fn empty_checked_contract_cannot_erase_implicit_parameter_ranges() {
    for parameter_type in ["u64 [0..=11]", "u64 [0..12]"] {
        let source = format!("machine enter(input: {parameter_type}) -> u64 {{ input }}");
        let original = checked(&source);
        let source_machine = &original.machines()[0];
        assert!(original.machine_contracts(source_machine).is_empty());
        let (artifact, module) = publish(&original);
        assert_eq!(module.machines.len(), 1);
        let contract = &module.machines[0].contract;
        assert_eq!(
            contract.requires.len(),
            1,
            "range is a real entry requirement"
        );
        assert!(!matches!(
            contract.requires[0],
            semantic_vocabulary::Proposition::Truth
        ));
        assert!(contract.ensures.is_empty());
        execute_identity(&artifact);
        reject_erased_contract(
            &original,
            "enter",
            "empty scalar contract would erase an authored parameter range",
        );
    }
}

#[test]
fn empty_checked_contract_cannot_erase_a_transitive_scalar_callee_guarantee() {
    let source = r#"
        machine identity(input: u64) -> u64
        ensures result == input
        { input }
        machine enter(input: u64) -> u64 {
            let answer: u64 = identity(input);
            answer
        }
    "#;
    let original = checked(source);
    let (artifact, module) = publish(&original);
    assert_eq!(module.machines.len(), 2);
    let callee = module
        .machines
        .iter()
        .find(|machine| machine.id != module.entry)
        .expect("transitive scalar callee");
    assert_eq!(callee.contract.ensures.len(), 1);
    execute_identity(&artifact);
    reject_erased_contract(
        &original,
        "identity",
        "empty scalar contract would erase an authored normal clause",
    );
}
