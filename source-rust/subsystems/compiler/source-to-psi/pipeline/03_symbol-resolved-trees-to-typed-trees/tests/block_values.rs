//! What resolution and typing hand stage 04 about a block-valued arm: the
//! typed arm value is a block that owns its statements, and its final value
//! names the block's own local. The program is
//! `05_prepared-typed-trees-to-lowered-program/tests/support/dispatch_arms.rs`;
//! stage 05's side of the handoff is that stage's `tests/block_values.rs`.
//!
//! The rule: `wiki/spec/language/expressions.md`, "A block introduces local
//! scope and cleanup edges, not a new machine declaration", and
//! `wiki/spec/language/state_contracts.md`, "Disjoint arms or blocks may
//! reuse names": a block local belongs to its block, not to the body.

#[path = "../../05_prepared-typed-trees-to-lowered-program/tests/support/dispatch_arms.rs"]
mod program;

use source_compiler::representations::source_state::expression::{ExpressionNode, MatchPattern};
use source_compiler::representations::source_state::statement::StatementNode;

#[test]
fn a_block_arm_owns_its_statements_and_its_value_names_its_local() {
    let trees = program::typed();
    let (block, found) = program::block_binding(&trees, "port");

    // The match's first arm is the block; the wildcard arm is a literal.
    let pick = trees.realized_machine_named("pick").unwrap();
    let state = trees.machine_states(pick).first().unwrap();
    let arms = trees
        .expression_table
        .iter_expressions()
        .find_map(|(_, node)| match node {
            ExpressionNode::Match(matched)
                if trees
                    .expression_table
                    .match_arms(matched.arms)
                    .iter()
                    .any(|arm| arm.value == block) =>
            {
                Some(trees.expression_table.match_arms(matched.arms).to_vec())
            }
            _ => None,
        })
        .expect("a match holds the block as an arm value");
    assert!(matches!(arms[0].pattern, MatchPattern::Value(_)));
    assert_eq!(arms[0].value, block);
    assert!(matches!(arms[1].pattern, MatchPattern::Wildcard));

    // The block binds `port` to 41 and delivers `port`.
    let [StatementNode::LocalData(port)] = trees.statement_table.statements(found.statements)
    else {
        panic!("the block holds one statement, its local")
    };
    assert!(matches!(
        trees.expression_table.expression(port.initial_value),
        ExpressionNode::Integer(value) if value.value_i64() == Some(41)
    ));
    let ExpressionNode::Name(tail) = trees.expression_table.expression(found.tail) else {
        panic!("the block's value is a name")
    };
    assert!(port.symbol.is_valid());
    assert_eq!(
        tail.symbol, port.symbol,
        "the value reads the block's local"
    );

    // The local is the block's: no statement of the body binds it.
    assert!(
        trees
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .filter_map(StatementNode::as_local_data)
            .all(|local| local.symbol != port.symbol),
        "the body does not bind the block's local"
    );
}
