//! Trailing call preservation is separate from checking its result type.

use super::{Lexer, lower_syntax_trees, parse_syntax_trees};
use symbol_resolved_trees::SymbolResolvedTrees;
use symbol_resolved_trees::expression::ExpressionNode;
use symbol_resolved_trees::statement::StatementNode;

fn resolved(source: &str) -> SymbolResolvedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize tail call");
    let syntax = parse_syntax_trees(&tokens).expect("parse tail call");
    lower_syntax_trees(&syntax).expect("resolve tail call")
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
fn unit_tail_calls_preserve_free_self_and_static_qualified_expressions() {
    for return_annotation in ["", " -> ()"] {
        for (declarations, entry, call, target) in [
            (
                "",
                "enter()",
                "finish(identity(true))",
                "machine finish(flag: bool) {}",
            ),
            (
                "data Root {}",
                "Root::enter(&self)",
                "self.finish(identity(true))",
                "machine Root::finish(&self, flag: bool) {}",
            ),
            (
                "data Root {} data Sink {}",
                "Root::enter()",
                "Sink::finish(identity(true))",
                "machine Sink::finish(flag: bool) {}",
            ),
        ] {
            let program = resolved(&format!(
                "{declarations}
                 machine {entry}{return_annotation} {{ {call} }}
                 {target}
                 machine identity(flag: bool) -> bool {{ flag }}"
            ));
            let entry_name = entry.split_once('(').unwrap().0;
            let [StatementNode::Expression(expression)] = statements(&program, entry_name) else {
                panic!(
                    "Unit-context tail must not manufacture a local or discard statement: {call}{return_annotation}"
                );
            };
            let expressions = &program.tables.bodies.expressions;
            let ExpressionNode::Call(outer) = expressions.expression(*expression) else {
                panic!("authored tail call");
            };
            let target_name = if call.starts_with("self.") {
                "Root::finish"
            } else if call.starts_with("Sink::") {
                "Sink::finish"
            } else {
                "finish"
            };
            let target = program
                .machines
                .iter()
                .find(|machine| machine.name.as_str() == target_name)
                .expect("forward-declared target");
            let target_state =
                program.machine_state(program.machine_state_handles(target.states)[0]);
            assert_eq!(outer.target_symbol, target_state.symbol);
            let [operand] = expressions.expression_handles(outer.arguments) else {
                panic!("one authored operand");
            };
            let ExpressionNode::Call(nested) = expressions.expression(*operand) else {
                panic!("nested operand remains within the authored call");
            };
            assert!(nested.target_symbol.is_valid());
            assert_eq!(nested.target.as_str(), "identity");
            let [argument] = expressions.expression_handles(nested.arguments) else {
                panic!("one nested operand");
            };
            assert!(matches!(
                expressions.expression(*argument),
                ExpressionNode::Boolean(true)
            ));
            if call.starts_with("self.") {
                let ExpressionNode::Name(receiver) = expressions.expression(outer.receiver) else {
                    panic!("self receiver survives");
                };
                assert!(
                    matches!(expressions.name_path_members(receiver.members), [member] if member.as_str() == "self")
                );
            }
        }
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
fn declared_scalar_tail_calls_keep_existing_value_normalization() {
    for (primitive, value) in [("bool", "true"), ("i32", "7i32"), ("f64", "2.0f64")] {
        let program = resolved(&format!(
            "machine enter() -> {primitive} {{ value() }}
             machine value() -> {primitive} {{ {value} }}"
        ));
        let [
            StatementNode::LocalData(local),
            StatementNode::Expression(returned),
        ] = statements(&program, "enter")
        else {
            panic!("declared scalar returns retain their existing binding route: {primitive}");
        };
        let expressions = &program.tables.bodies.expressions;
        assert!(
            matches!(expressions.expression(local.initial_value), ExpressionNode::Call(call) if call.target.as_str() == "value")
        );
        assert!(
            matches!(expressions.expression(*returned), ExpressionNode::Name(name) if name.symbol == local.symbol)
        );
    }
}
