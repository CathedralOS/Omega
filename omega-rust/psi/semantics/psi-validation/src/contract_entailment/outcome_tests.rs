use super::*;
use crate::proven_machine_contract_expressions;
use psi_source_files_to_tokens::Lexer;
use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use psi_tokens_to_syntax_trees::parse_syntax_trees;

fn parse(source: &str) -> TypedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    lower_symbol_resolved_trees(&resolved).expect("type")
}

#[test]
fn positive_outcomes_keep_exact_machine_and_expression_identity() {
    let program = parse(
        "machine first(value: u64) ensures value == value {}\n\
         machine second(value: u64) ensures value == value {}",
    );
    let first = &program.machines()[0];
    let second = &program.machines()[1];
    let expected = program
        .machine_contracts(first)
        .iter()
        .find_map(|contract| {
            program
                .proof_facts
                .span_or_empty(contract.facts)
                .iter()
                .find_map(|fact| {
                    let ProofFact::Expression(expression) = fact else {
                        return None;
                    };
                    Some(*expression)
                })
        })
        .expect("first postcondition");
    assert_eq!(
        proven_machine_contract_expressions(&program, first.symbol),
        vec![expected]
    );
    assert!(!proven_machine_contract_expressions(&program, second.symbol).contains(&expected));
}

#[test]
fn unsupported_body_does_not_become_a_positive_outcome() {
    let program =
        parse("machine theorem(value: u64) ensures value == value { let unused: u64 = 1; }");
    assert!(proven_machine_contract_expressions(&program, program.machines()[0].symbol).is_empty());
    assert!(!crate::collect_contract_entailment_stand_downs(&program).is_empty());
}

#[test]
fn membership_is_not_boolean_entailment_evidence() {
    let program = parse(
        "domain u64::Small requires self < 10; machine theorem(value: u64) ensures value in Small {}",
    );
    assert!(proven_machine_contract_expressions(&program, program.machines()[0].symbol).is_empty());
}

#[test]
fn a_refuted_conjunct_prevents_exporting_partial_success() {
    let program = parse("machine theorem(value: u64) ensures value == value\n1 == 2\n{}");
    assert!(proven_machine_contract_expressions(&program, program.machines()[0].symbol).is_empty());
}

#[test]
fn declarations_and_admissions_do_not_supply_checked_body_outcomes() {
    let mut program = parse("machine theorem(value: u64) ensures value == value {}");
    let symbol = program.machines()[0].symbol;
    for mode in [
        psi_language_semantics::MachineSupplyMode::Boundary,
        psi_language_semantics::MachineSupplyMode::AdmissionClaim,
        psi_language_semantics::MachineSupplyMode::TopLevelRequirement,
    ] {
        program.machines_mut()[0].supply_mode = mode;
        assert!(
            proven_machine_contract_expressions(&program, symbol).is_empty(),
            "{mode:?}"
        );
    }
}

#[test]
fn a_proved_guarded_arm_does_not_prove_an_implicit_fallthrough() {
    let mut program = parse(
        "machine theorem(value: u64) -> u64 ensures value > 0 { transition value > 0 { true -> (1) false -> (0) } }",
    );
    let machine = program.machines()[0].clone();
    let state = program.machine_states(&machine)[0].clone();
    let first = program.statement_table.statements(state.statement_nodes)[0].clone();
    let mut statements = psi_arena::HandleSpan::default();
    program
        .statement_table
        .push_statement(&mut statements, first);
    program.machine_states_mut(&machine)[0].statement_nodes = statements;
    assert!(proven_machine_contract_expressions(&program, program.machines()[0].symbol).is_empty());
}
