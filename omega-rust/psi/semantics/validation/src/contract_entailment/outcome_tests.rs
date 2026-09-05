use super::*;
use crate::proven_machine_contract_expressions;
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use tokens_to_syntax_trees::parse_syntax_trees;

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
        language_semantics::MachineSupplyMode::Boundary,
        language_semantics::MachineSupplyMode::AdmissionClaim,
        language_semantics::MachineSupplyMode::TopLevelRequirement,
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
    let mut statements = arena::HandleSpan::default();
    program
        .statement_table
        .push_statement(&mut statements, first);
    program.machine_states_mut(&machine)[0].statement_nodes = statements;
    assert!(proven_machine_contract_expressions(&program, program.machines()[0].symbol).is_empty());
}

#[test]
fn exhaustive_computed_proof_subject_requires_exact_call_identity() {
    let mut program = parse(
        r#"
        data Nat { case Zero; case Succ(prev: Nat); }
        machine identity(value: Nat) -> Nat { transition { _ -> value } }
        machine other(value: Nat) -> Nat { transition { _ -> value } }
        machine theorem(value: Nat) ensures value == value {
            transition identity(value) {
                Nat::Zero -> base()
                Nat::Succ { prev } -> step(prev)
            }
            state base() {}
            state step(prev: Nat) {}
        }
    "#,
    );
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "theorem")
        .expect("theorem")
        .clone();
    assert!(entailment_covers_all_exits(&program, &machine));
    let calls: Vec<_> = program
        .statement_table
        .statements(program.machine_states(&machine)[0].statement_nodes)
        .iter()
        .filter_map(|statement| {
            let StatementNode::Transition(transition) = statement else {
                return None;
            };
            let TransitionGuardNode::When(guard) = transition.guard else {
                return None;
            };
            let ExpressionNode::Binary(comparison) = program.expression_table.expression(guard)
            else {
                return None;
            };
            Some(comparison.left)
        })
        .collect();
    assert_eq!(calls.len(), 2);
    let other = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "other")
        .expect("other");
    let other_symbol = program.machine_states(other)[0].symbol;
    let original = program.expression_table.expression(calls[1]).clone();
    let ExpressionNode::Call(call) = program.expression_table.expression_mut(calls[1]) else {
        panic!("computed condition");
    };
    call.target_symbol = other_symbol;
    assert!(
        !entailment_covers_all_exits(&program, &machine),
        "different calls do not cover one subject"
    );
    *program.expression_table.expression_mut(calls[1]) = original;
    let ExpressionNode::Call(call) = program.expression_table.expression_mut(calls[1]) else {
        unreachable!();
    };
    call.machine_arguments = vec![typed_trees::expression::StaticMachineArgument {
        path: Box::new([]),
        application: None,
        const_literal: None,
        evidence_projection: None,
        symbol: other_symbol,
    }]
    .into_boxed_slice();
    assert!(
        !entailment_covers_all_exits(&program, &machine),
        "static argument drift changes the computed subject"
    );
}

#[test]
fn inherited_law_matches_retain_exact_authored_proof_roots() {
    let program = parse(
        r#"
        trait Reflexive { machine law(value: u64) ensures value == value; }
        machine theorem(value: u64) satisfies Reflexive::law ensures value == value {}
    "#,
    );
    let machine = &program.machines()[0];
    let proven = proven_machine_contract_expressions(&program, machine.symbol);
    let matches = crate::matched_machine_law_guarantees(&program, machine.symbol);
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].machine, machine.symbol);
    assert_ne!(matches[0].expression, matches[0].source_expressions[0]);
    assert_eq!(matches[0].source_expressions, proven);
}

#[test]
fn inherited_law_matching_does_not_prove_a_false_authored_claim() {
    let program = parse(
        r#"
        trait EqualityClaim { machine law(left: u64, right: u64) ensures left == right; }
        machine theorem(left: u64, right: u64) satisfies EqualityClaim::law ensures left == right {}
    "#,
    );
    let machine = &program.machines()[0];
    assert_eq!(
        crate::matched_machine_law_guarantees(&program, machine.symbol).len(),
        1
    );
    assert!(
        proven_machine_contract_expressions(&program, machine.symbol).is_empty(),
        "conformance matching is correspondence, not a proof of the claim"
    );
}

#[test]
fn inherited_law_matching_rejects_exact_requirement_drift() {
    let mut program = parse(
        r#"
        trait Reflexive {
            machine law(value: u64) ensures value == value;
            machine other(value: u64) ensures value == value;
        }
        machine theorem(value: u64) satisfies Reflexive::law ensures value == value {}
    "#,
    );
    let machine = program.machines()[0].clone();
    let other = program
        .trait_machine_signatures(&program.traits()[0])
        .iter()
        .find(|requirement| requirement.name.as_str() == "other")
        .expect("other requirement")
        .symbol;
    let conformance = program.machine_trait_conformances(&machine)[0].clone();
    let handle = program
        .machine_trait_conformances
        .iter()
        .find_map(|(handle, candidate)| (candidate == &conformance).then_some(handle))
        .expect("conformance handle");
    program
        .machine_trait_conformances
        .get_mut(handle)
        .requirement_symbol = other;
    assert!(crate::matched_machine_law_guarantees(&program, machine.symbol).is_empty());
}
