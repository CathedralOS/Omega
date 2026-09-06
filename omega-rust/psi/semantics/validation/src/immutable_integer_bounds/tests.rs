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
            immutable_integer_bound_value_symbol(&program, expression),
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
    assert!(immutable_integer_bound_value_symbol(&program, literal).is_none());
    assert!(immutable_integer_bound_value_symbol(&program, computed).is_none());
    assert!(immutable_integer_bound_value_symbol(&program, unresolved).is_none());
    assert!(normalize_immutable_integer_bound_expression(&program, unresolved).is_none());
}

#[test]
fn immutable_copies_of_mutable_sources_are_values_not_static_indexes() {
    for parameter_source in [false, true] {
        let mut program = TypedTrees::default();
        let original_symbol = SymbolHandle::from_arena_index(1);
        let original = name(&mut program, "original", original_symbol);
        let cut_symbol = SymbolHandle::from_arena_index(2);
        let cut = name(&mut program, "cut", cut_symbol);
        let copy_symbol = SymbolHandle::from_arena_index(3);
        let copy = name(&mut program, "copy", copy_symbol);
        let later_symbol = SymbolHandle::from_arena_index(4);
        let later = name(&mut program, "later", later_symbol);
        let one = program.expression_table.insert(ExpressionNode::Integer(
            numerics::literals::IntegerLiteral::from_value(1),
        ));
        let mut machine = Machine::default();
        let mut state = State::default();
        if parameter_source {
            program.push_state_parameter(
                &mut state,
                typed_trees::signature::StateParameter {
                    symbol: original_symbol,
                    is_mutable: true,
                    ..Default::default()
                },
            );
        } else {
            program.statement_table.push_statement(
                &mut state.statement_nodes,
                StatementNode::LocalData(TableLocalData {
                    symbol: original_symbol,
                    name: Identifier::generated_static("original"),
                    initial_value: one,
                    is_mutable: true,
                    ..Default::default()
                }),
            );
        }
        for (symbol, spelling, initial_value) in [
            (cut_symbol, "cut", original),
            (copy_symbol, "copy", cut),
            (later_symbol, "later", original),
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
            (original, None),
            (cut, Some(cut_symbol)),
            (copy, Some(cut_symbol)),
            (later, Some(later_symbol)),
        ] {
            assert_eq!(
                immutable_integer_bound_value_symbol(&program, expression),
                expected
            );
            assert!(normalize_immutable_integer_bound_expression(&program, expression).is_none());
            assert!(normalize_immutable_integer_bound_to_usize(&program, expression).is_none());
        }
        // Ambiguous mutable origins must not become stable snapshot fallbacks.
        let mut duplicate_machine = Machine::default();
        let mut duplicate_state = State::default();
        if parameter_source {
            program.push_state_parameter(
                &mut duplicate_state,
                typed_trees::signature::StateParameter {
                    symbol: original_symbol,
                    is_mutable: false,
                    ..Default::default()
                },
            );
        } else {
            program.statement_table.push_statement(
                &mut duplicate_state.statement_nodes,
                StatementNode::LocalData(TableLocalData {
                    symbol: original_symbol,
                    initial_value: one,
                    ..Default::default()
                }),
            );
        }
        program.push_machine_state(&mut duplicate_machine, duplicate_state);
        program.push_machine(duplicate_machine);
        for expression in [original, cut, copy, later] {
            assert!(immutable_integer_bound_value_symbol(&program, expression).is_none());
            assert!(normalize_immutable_integer_bound_expression(&program, expression).is_none());
            assert!(normalize_immutable_integer_bound_to_usize(&program, expression).is_none());
        }
    }
}
