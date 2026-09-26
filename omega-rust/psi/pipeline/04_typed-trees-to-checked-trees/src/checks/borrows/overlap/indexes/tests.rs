//! Index overlap tests: selector snapshots, evaluated extents and replay drift.

use super::{ExpressionHandle, ExpressionNode, SymbolHandle, TableRangeExpression};
use crate::checks::borrows::overlap::indexes::index_expressions_may_overlap;
use typed_trees::expression::{BinaryOperator, Expression, NamePath, TableBinaryExpression};
use typed_trees::machine::Machine;
use typed_trees::name::Identifier;
use typed_trees::state::State;
use typed_trees::statement::{StatementNode, TableLocalData};

fn symbol(index: u32) -> SymbolHandle {
    SymbolHandle::from_arena_index(index)
}

fn integer(program: &mut typed_trees::TypedTrees, value: i64) -> ExpressionHandle {
    program.expression_table.insert(ExpressionNode::Integer(
        numerics::literals::IntegerLiteral::from_value(value),
    ))
}

fn range(
    program: &mut typed_trees::TypedTrees,
    start: i64,
    end: i64,
    end_inclusive: bool,
) -> ExpressionHandle {
    let start = integer(program, start);
    let end = integer(program, end);
    program
        .expression_table
        .insert(ExpressionNode::Range(TableRangeExpression {
            start,
            end,
            end_inclusive,
        }))
}

fn named_bound(
    program: &mut typed_trees::TypedTrees,
    name: &'static str,
    symbol: SymbolHandle,
) -> ExpressionHandle {
    program
        .expression_table
        .insert_tree(&Expression::Name(NamePath::resolved(
            vec![Identifier::generated_static(name)],
            symbol,
            symbol,
        )))
}

fn exact_integer_type(
    program: &mut typed_trees::TypedTrees,
) -> typed_trees::types::TypeReferenceHandle {
    program
        .type_reference_table
        .insert(typed_trees::types::TypeReferenceNode::Named {
            symbol: SymbolHandle::invalid(),
            name: Identifier::generated_static("u64"),
        })
}

fn offset_bound(
    program: &mut typed_trees::TypedTrees,
    base: ExpressionHandle,
    operator: BinaryOperator,
    offset: i64,
) -> ExpressionHandle {
    let literal = integer(program, offset);
    program
        .expression_table
        .insert(ExpressionNode::Binary(TableBinaryExpression {
            left: base,
            operator,
            right: literal,
        }))
}

fn range_bounds(
    program: &mut typed_trees::TypedTrees,
    start: ExpressionHandle,
    end: ExpressionHandle,
    end_inclusive: bool,
) -> ExpressionHandle {
    program
        .expression_table
        .insert(ExpressionNode::Range(TableRangeExpression {
            start,
            end,
            end_inclusive,
        }))
}

fn install_locals(
    program: &mut typed_trees::TypedTrees,
    locals: impl IntoIterator<Item = (SymbolHandle, &'static str, ExpressionHandle, bool)>,
) {
    let type_reference = exact_integer_type(program);
    install_locals_with_type(program, locals, type_reference);
}

fn install_locals_with_type(
    program: &mut typed_trees::TypedTrees,
    locals: impl IntoIterator<Item = (SymbolHandle, &'static str, ExpressionHandle, bool)>,
    type_reference: typed_trees::types::TypeReferenceHandle,
) {
    let mut machine = Machine::default();
    let mut state = State::default();
    for (symbol, name, initial_value, is_mutable) in locals {
        program.statement_table.push_statement(
            &mut state.statement_nodes,
            StatementNode::LocalData(TableLocalData {
                symbol,
                name: Identifier::generated_static(name),
                type_reference,
                initial_value,
                is_mutable,
                ..Default::default()
            }),
        );
    }
    program.push_machine_state(&mut machine, state);
    program.push_machine(machine);
}

#[test]
fn shared_symbol_offsets_order_slice_windows() {
    let mut program = typed_trees::TypedTrees::default();
    let mid_symbol = symbol(30);
    let mid = named_bound(&mut program, "mid", mid_symbol);
    let mid_plus_one = offset_bound(&mut program, mid, BinaryOperator::Add, 1);
    let mid_plus_two = offset_bound(&mut program, mid, BinaryOperator::Add, 2);
    let mid_minus_one = offset_bound(&mut program, mid, BinaryOperator::Subtract, 1);
    let other = named_bound(&mut program, "other", symbol(31));
    let other_plus_one = offset_bound(&mut program, other, BinaryOperator::Add, 1);
    let zero = integer(&mut program, 0);
    let four = integer(&mut program, 4);
    let one = integer(&mut program, 1);
    let mid_initial =
        program
            .expression_table
            .insert(ExpressionNode::Binary(TableBinaryExpression {
                left: one,
                operator: BinaryOperator::Add,
                right: one,
            }));
    install_locals(&mut program, [(mid_symbol, "mid", mid_initial, false)]);
    let mid_range = range_bounds(&mut program, zero, mid, false);
    let inclusive_mid_range = range_bounds(&mut program, zero, mid, true);
    let plus_one_range = range_bounds(&mut program, mid_plus_one, four, false);
    let plus_two_range = range_bounds(&mut program, zero, mid_plus_two, false);
    let minus_one_range = range_bounds(&mut program, mid_minus_one, four, false);
    let other_plus_one_range = range_bounds(&mut program, other_plus_one, four, false);

    assert!(!index_expressions_may_overlap(
        &program,
        mid_range,
        plus_one_range
    ));
    assert!(!index_expressions_may_overlap(
        &program,
        inclusive_mid_range,
        plus_one_range
    ));
    assert!(!index_expressions_may_overlap(
        &program,
        plus_one_range,
        inclusive_mid_range
    ));
    assert!(index_expressions_may_overlap(
        &program,
        plus_two_range,
        plus_one_range
    ));
    assert!(index_expressions_may_overlap(
        &program,
        mid_range,
        minus_one_range
    ));
    let empty = range_bounds(&mut program, mid_plus_one, mid, false);
    let full = range_bounds(&mut program, zero, four, false);
    assert!(!index_expressions_may_overlap(&program, empty, full));
    assert!(index_expressions_may_overlap(
        &program,
        mid_range,
        other_plus_one_range
    ));
}

#[test]
fn empty_inclusive_window_overlaps_nothing() {
    let mut program = typed_trees::TypedTrees::default();
    // `2..=1` normalizes to `[2, 2)` -- empty, disjoint from index 2.
    let window = range(&mut program, 2, 1, true);
    let index = integer(&mut program, 2);
    assert!(!index_expressions_may_overlap(&program, window, index));
}
