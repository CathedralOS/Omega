//! Trailing call preservation is separate from checking its result type.

use crate::resolution::{ResolutionRequest, resolve};
use source_files_to_tokens::Lexer;
use symbol_resolved_trees::SymbolResolvedTrees;
use symbol_resolved_trees::expression::ExpressionNode;
use symbol_resolved_trees::statement::StatementNode;
use tokens_to_syntax_trees::parse_syntax_trees;

fn resolved(source: &str) -> SymbolResolvedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize tail call");
    let syntax = parse_syntax_trees(&tokens).expect("parse tail call");
    resolve(ResolutionRequest::new(&syntax)).expect("resolve tail call")
}

fn statements<'program>(
    program: &'program SymbolResolvedTrees,
    name: &str,
) -> &'program [StatementNode] {
    let machine = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == name)
        .expect("source machine");
    let state = program.machine_state(program.machine_state_handles(machine.states)[0]);
    program
        .tables
        .bodies
        .statements
        .statements(state.statement_nodes)
}

#[test]
fn unit_context_preservation_does_not_reclassify_value_calls_as_discard_statements() {
    for return_annotation in ["", " -> ()"] {
        let program = resolved(&format!(
            "machine enter(){return_annotation} {{ value() }}
             machine value() -> bool {{ true }}"
        ));
        let [StatementNode::Expression(expression)] = statements(&program, "enter") else {
            panic!("checking, not normalization, rejects the non-Unit result");
        };
        assert!(
            matches!(program.tables.bodies.expressions.expression(*expression), ExpressionNode::Call(call) if call.target.as_str() == "value")
        );
    }
}

#[test]
fn qualified_self_tail_calls_resolve_to_the_current_entry_only() {
    use symbol_resolved_trees::statement::TransitionTargetNode;
    let program = resolved(
        "data Walker {} data Other {}
         machine Walker::walk(value: u64) -> u64 {
             transition value < 3 { true -> step(value) false -> (value) }
             state step(value: u64) -> u64 { Walker::walk(value) }
         }
         machine Other::walk(value: u64) -> u64 { Walker::walk(value) }",
    );
    let walker = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "Walker::walk")
        .unwrap();
    let state = program.machine_state(program.machine_state_handles(walker.states)[1]);
    let [StatementNode::Transition(transition)] = program
        .tables
        .bodies
        .statements
        .statements(state.statement_nodes)
    else {
        panic!("qualified self tail call should become a transition");
    };
    let TransitionTargetNode::Named { path, .. } = program
        .tables
        .bodies
        .statements
        .transition_target(transition.target)
    else {
        panic!("qualified self tail call should name the current entry");
    };
    assert_eq!(
        path.symbol,
        program
            .machine_state(program.machine_state_handles(walker.states)[0])
            .symbol
    );
    let [StatementNode::Expression(expression)] = statements(&program, "Other::walk") else {
        panic!("same leaf name in another machine must remain an ordinary call");
    };
    assert!(matches!(
        program.tables.bodies.expressions.expression(*expression),
        ExpressionNode::Call(_)
    ));
}

#[test]
fn qualified_self_call_inside_arithmetic_keeps_its_call_identity() {
    let program = resolved(
        "data Walker {} machine Walker::walk(value: u64) -> u64 { 3 * Walker::walk(value) }",
    );
    let body = statements(&program, "Walker::walk");
    assert!(
        !body
            .iter()
            .any(|statement| matches!(statement, StatementNode::Transition(_)))
    );
}

#[test]
fn qualified_self_value_arm_preserves_guard_and_authored_ranking() {
    use symbol_resolved_trees::statement::{TransitionGuardNode, TransitionTargetNode};
    let program = resolved(
        "data Walker {}
         machine Walker::walk(value: u64) -> u64 terminates by value; {
             transition value > 0 {
                 true -> (Walker::walk(value - 1))
                 false -> (value)
             }
         }",
    );
    let machine = &program.machines[0];
    assert!(!machine.ranking_subjects.is_empty());
    let StatementNode::Transition(transition) = &statements(&program, "Walker::walk")[0] else {
        panic!("guarded return");
    };
    assert!(matches!(transition.guard, TransitionGuardNode::When(_)));
    assert!(matches!(
        program
            .tables
            .bodies
            .statements
            .transition_target(transition.target),
        TransitionTargetNode::Named { .. }
    ));
}

#[test]
fn call_on_another_instance_is_not_an_activation_backedge() {
    let program = resolved(
        "data Walker {}
         machine Walker::walk(&self, other: &Walker, value: u64) -> u64 {
             other.walk(other, value)
         }",
    );
    assert!(matches!(
        statements(&program, "Walker::walk"),
        [StatementNode::Expression(_)]
    ));
}

#[test]
fn self_tail_result_mismatch_keeps_the_value_call_for_type_checking() {
    for result in ["bool", "u64 in Wrapping"] {
        let program = resolved(&format!(
            "data Walker {{}}
             machine Walker::walk(value: u64) -> u64 {{
                 transition {{ _ -> step(value) }}
                 state step(value: u64) -> {result} {{ Walker::walk(value) }}
             }}"
        ));
        let machine = &program.machines[0];
        let state = program.machine_state(program.machine_state_handles(machine.states)[1]);
        assert!(
            matches!(
                program
                    .tables
                    .bodies
                    .statements
                    .statements(state.statement_nodes),
                [StatementNode::Expression(_)]
            ),
            "{result}: preserve result checking"
        );
    }
}
