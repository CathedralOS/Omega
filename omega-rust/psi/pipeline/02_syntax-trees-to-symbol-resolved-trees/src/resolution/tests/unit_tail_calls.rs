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
