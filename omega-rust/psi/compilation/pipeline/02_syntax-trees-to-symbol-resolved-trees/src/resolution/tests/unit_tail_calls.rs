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
    resolve(ResolutionRequest::new(syntax)).expect("resolve tail call")
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
fn ordinary_call_and_internal_jump_have_distinct_target_namespaces() {
    use symbol_resolved_trees::statement::TransitionTargetNode;
    let program = resolved(
        "machine finish(value: u64) -> u64 { value }
         machine run() -> u64 {
             let value: u64 = finish(42);
             transition { _ -> finish(value) }
             state finish(value: u64) -> u64 { value }
         }",
    );
    let [
        StatementNode::LocalData(local),
        StatementNode::Transition(transition),
    ] = statements(&program, "run")
    else {
        panic!("initializer followed by internal jump");
    };
    let ExpressionNode::Call(call) = program
        .tables
        .bodies
        .expressions
        .expression(local.initial_value)
    else {
        panic!("ordinary initializer call");
    };
    let TransitionTargetNode::Named { path, .. } = program
        .tables
        .bodies
        .statements
        .transition_target(transition.target)
    else {
        panic!("internal state destination");
    };
    assert!(call.target_symbol.is_valid());
    assert!(path.symbol.is_valid());
    assert_ne!(call.target_symbol, path.symbol);
    let machine = |name: &str| {
        program
            .machines
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .expect("machine")
            .symbol
    };
    assert_eq!(
        program.symbols.get(call.target_symbol).parent,
        machine("finish")
    );
    assert_eq!(program.symbols.get(path.symbol).parent, machine("run"));
}

#[test]
fn internal_states_are_not_ordinary_call_candidates() {
    for spelling in ["finish(42)", "self.finish(42)", "Helper::run::finish(42)"] {
        let program = resolved(&format!(
            "data Helper {{}}
             machine Helper::run(&self) -> u64 {{
                 let value: u64 = {spelling};
                 transition {{ _ -> finish(value) }}
                 state finish(&self, value: u64) -> u64 {{ value }}
             }}"
        ));
        let StatementNode::LocalData(local) = &statements(&program, "Helper::run")[0] else {
            panic!("initializer");
        };
        let ExpressionNode::Call(call) = program
            .tables
            .bodies
            .expressions
            .expression(local.initial_value)
        else {
            panic!("ordinary initializer call");
        };
        assert!(
            !call.target_symbol.is_valid(),
            "{spelling} selected a private state"
        );
    }
}

#[test]
fn trait_contract_calls_keep_their_requirement_instead_of_a_free_machine() {
    let program = resolved(
        "machine zero() -> u64 { 0 }
         trait Numbers {
             machine zero() -> u64;
             machine law() ensures zero() == zero();
         }",
    );
    let calls = program
        .tables
        .bodies
        .expressions
        .iter_expressions()
        .filter_map(|(_, expression)| match expression {
            ExpressionNode::Call(call) if call.target.as_str() == "zero" => Some(call),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(!calls.is_empty(), "trait law retains its calls");
    for call in calls {
        assert!(call.target_symbol.is_valid());
        let owner = program.symbols.get(call.target_symbol).parent;
        assert_eq!(program.symbols.get(owner).kind, symbols::SymbolKind::Trait);
        assert_eq!(program.symbols.name(owner), "Numbers");
    }
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
