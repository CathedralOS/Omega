use super::{StructuralJudge, recognize_guarded_structural_value_arms};
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees::TypedTrees;
use typed_trees::statement::StatementNode;

fn parse(body: &str) -> TypedTrees {
    let source = format!(
        "data Nat {{ case Zero; case Succ(prev: Nat); }}
         machine theorem(value: u64) -> Nat ensures result == Nat::Zero {{ {body} }}"
    );
    let tokens = Lexer::new(&source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    lower_symbol_resolved_trees(&resolved).expect("type")
}

#[test]
fn boolean_fallthrough_checks_both_structural_results_in_either_order() {
    for body in [
        "transition value > 0 { true -> Nat::Zero false -> Nat::Zero }",
        "transition value > 0 { false -> Nat::Zero true -> Nat::Zero }",
    ] {
        let program = parse(body);
        let machine = &program.machines()[0];
        let judge = StructuralJudge::from_requires(&program, machine, &[]);
        let arms = recognize_guarded_structural_value_arms(&program, machine, &judge)
            .expect("conditional value and unconditional fallback cover both results");
        assert_eq!(arms.len(), 2);
        assert_eq!(
            crate::proven_machine_contract_expressions(&program, machine.symbol).len(),
            1
        );
    }
}

#[test]
fn false_structural_result_on_either_boolean_arm_is_not_proven() {
    for body in [
        "transition value > 0 { true -> Nat::Succ { prev: Nat::Zero } false -> Nat::Zero }",
        "transition value > 0 { true -> Nat::Zero false -> Nat::Succ { prev: Nat::Zero } }",
    ] {
        let program = parse(body);
        assert!(
            crate::proven_machine_contract_expressions(&program, program.machines()[0].symbol)
                .is_empty()
        );
    }
}

#[test]
fn partial_or_repeated_boolean_arms_do_not_supply_exhaustive_structural_results() {
    for repeat_first_guard in [false, true] {
        // Typing already rejects authored fallthrough. Corrupt a typed total
        // dispatch to exercise this proof reader's independent coverage check.
        let mut program = parse("transition value > 0 { true -> Nat::Zero false -> Nat::Zero }");
        let machine = program.machines()[0].clone();
        let state = program.machine_states(&machine)[0].clone();
        let first = program.statement_table.statements(state.statement_nodes)[0].clone();
        if repeat_first_guard {
            let StatementNode::Transition(first) = first else {
                panic!("first transition");
            };
            let StatementNode::Transition(second) = &mut program
                .statement_table
                .statements_mut(state.statement_nodes)[1]
            else {
                panic!("fallback transition");
            };
            second.guard = first.guard;
        } else {
            let mut statements = arena::HandleSpan::default();
            program
                .statement_table
                .push_statement(&mut statements, first);
            program.machine_states_mut(&machine)[0].statement_nodes = statements;
        }
        let judge = StructuralJudge::from_requires(&program, &machine, &[]);
        assert!(recognize_guarded_structural_value_arms(&program, &machine, &judge).is_none());
        assert!(crate::proven_machine_contract_expressions(&program, machine.symbol).is_empty());
    }
}
