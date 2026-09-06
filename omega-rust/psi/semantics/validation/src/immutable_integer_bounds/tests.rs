//! Stable computed values are not compile-time integer constants.

use super::*;
use typed_trees::expression::{BinaryOperator, Expression, NamePath, TableBinaryExpression};
use typed_trees::machine::Machine;
use typed_trees::name::Identifier;
use typed_trees::state::State;

fn name(
    program: &mut TypedTrees,
    spelling: &'static str,
    symbol: SymbolHandle,
) -> ExpressionHandle {
    program
        .expression_table
        .insert_tree(&Expression::Name(NamePath::resolved(
            vec![Identifier::generated_static(spelling)],
            symbol,
            symbol,
        )))
}

#[test]
fn computed_identity_preserves_copy_chains_without_becoming_a_static_index() {
    let mut program = TypedTrees::default();
    let one = program.expression_table.insert(ExpressionNode::Integer(
        numerics::literals::IntegerLiteral::from_value(1),
    ));
    let computed = program
        .expression_table
        .insert(ExpressionNode::Binary(TableBinaryExpression {
            left: one,
            operator: BinaryOperator::Add,
            right: one,
        }));
    let first_symbol = SymbolHandle::from_arena_index(1);
    let first = name(&mut program, "first", first_symbol);
    let copy_symbol = SymbolHandle::from_arena_index(2);
    let copy = name(&mut program, "copy", copy_symbol);
    let distinct_symbol = SymbolHandle::from_arena_index(3);
    let distinct = name(&mut program, "distinct", distinct_symbol);
    let literal_symbol = SymbolHandle::from_arena_index(4);
    let literal = name(&mut program, "literal", literal_symbol);
    let unresolved = name(&mut program, "ambiguous", SymbolHandle::invalid());
    let mut machine = Machine::default();
    let mut state = State::default();
    for (symbol, spelling, initial_value) in [
        (first_symbol, "first", computed),
        (copy_symbol, "copy", first),
        (distinct_symbol, "distinct", computed),
        (literal_symbol, "literal", one),
        (SymbolHandle::from_arena_index(5), "ambiguous", computed),
        (SymbolHandle::from_arena_index(6), "ambiguous", computed),
    ] {
        program.statement_table.push_statement(
            &mut state.statement_nodes,
            StatementNode::LocalData(TableLocalData {
                symbol,
                name: Identifier::generated_static(spelling),
                initial_value,
                ..Default::default()
            }),
        );
    }
    program.push_machine_state(&mut machine, state);
    program.push_machine(machine);
    for (expression, expected) in [
        (first, first_symbol),
        (copy, first_symbol),
        (distinct, distinct_symbol),
    ] {
        assert_eq!(
            computed_immutable_integer_bound_symbol(&program, expression),
            Some(expected)
        );
        assert!(normalize_immutable_integer_bound_expression(&program, expression).is_none());
        assert!(normalize_immutable_integer_bound_to_usize(&program, expression).is_none());
    }
    assert_eq!(
        normalize_immutable_integer_bound_to_usize(&program, literal),
        Some(1)
    );
    assert_eq!(
        normalize_immutable_integer_bound_expression(&program, literal),
        Some(one)
    );
    assert!(computed_immutable_integer_bound_symbol(&program, literal).is_none());
    assert!(computed_immutable_integer_bound_symbol(&program, computed).is_none());
    assert!(computed_immutable_integer_bound_symbol(&program, unresolved).is_none());
    assert!(normalize_immutable_integer_bound_expression(&program, unresolved).is_none());
}
