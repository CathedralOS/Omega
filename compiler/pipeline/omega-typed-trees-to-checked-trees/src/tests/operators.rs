use super::*;
use omega_core::operator_spelling::OperatorSpelling;
use omega_typed_trees::expression::{ExpressionNode, TableIndexedExpression, TableRangeExpression};
use omega_typed_trees::operator::OperatorDefinition;

#[test]
fn records_indexed_expression_operator_spelling_resolution() {
    let index_operator_symbol = SymbolHandle::from_arena_index(80);
    let range_operator_symbol = SymbolHandle::from_arena_index(81);

    let mut program = omega_typed_trees::TypedTrees::default();
    program.push_operator(operator_with_spelling(
        index_operator_symbol,
        OperatorSpelling::Index,
    ));
    program.push_operator(operator_with_spelling(
        range_operator_symbol,
        OperatorSpelling::Range,
    ));

    let collection = program.expression_table.insert(ExpressionNode::Integer(0));
    let index = program.expression_table.insert(ExpressionNode::Integer(0));
    let indexed =
        program
            .expression_table
            .insert(ExpressionNode::Indexed(TableIndexedExpression {
                collection,
                index,
            }));

    let range_start = program.expression_table.insert(ExpressionNode::Integer(0));
    let range_end = program.expression_table.insert(ExpressionNode::Integer(1));
    let range = program
        .expression_table
        .insert(ExpressionNode::Range(TableRangeExpression {
            start: range_start,
            end: range_end,
            end_inclusive: false,
        }));
    let ranged = program
        .expression_table
        .insert(ExpressionNode::Indexed(TableIndexedExpression {
            collection,
            index: range,
        }));

    let values = checked_values_for([indexed, ranged]);

    let facts = build_operator_facts(&program, &values);
    let indexed_use = facts.expression_use(indexed).expect("indexed use");
    let ranged_use = facts.expression_use(ranged).expect("ranged use");

    assert_eq!(indexed_use.spelling, OperatorSpelling::Index);
    assert_eq!(indexed_use.selected_operator_symbol, index_operator_symbol);
    assert_eq!(
        indexed_use.status,
        omega_checked_trees::CheckedOperatorResolutionStatus::Resolved
    );
    assert_eq!(ranged_use.spelling, OperatorSpelling::Range);
    assert_eq!(ranged_use.selected_operator_symbol, range_operator_symbol);
    assert_eq!(
        ranged_use.status,
        omega_checked_trees::CheckedOperatorResolutionStatus::Resolved
    );
}

#[test]
fn records_ambiguous_operator_spelling_status() {
    let mut program = omega_typed_trees::TypedTrees::default();
    program.push_operator(operator_with_spelling(
        SymbolHandle::from_arena_index(90),
        OperatorSpelling::Index,
    ));
    program.push_operator(operator_with_spelling(
        SymbolHandle::from_arena_index(91),
        OperatorSpelling::Index,
    ));

    let collection = program.expression_table.insert(ExpressionNode::Integer(0));
    let index = program.expression_table.insert(ExpressionNode::Integer(0));
    let indexed =
        program
            .expression_table
            .insert(ExpressionNode::Indexed(TableIndexedExpression {
                collection,
                index,
            }));

    let values = checked_values_for([indexed]);
    let facts = build_operator_facts(&program, &values);
    let indexed_use = facts.expression_use(indexed).expect("indexed use");

    assert_eq!(indexed_use.spelling, OperatorSpelling::Index);
    assert_eq!(
        indexed_use.status,
        omega_checked_trees::CheckedOperatorResolutionStatus::Ambiguous
    );
    assert_eq!(indexed_use.candidate_count, 2);
    assert!(!indexed_use.selected_operator_symbol.is_valid());
}

fn checked_values_for(
    expressions: impl IntoIterator<Item = omega_typed_trees::expression::ExpressionHandle>,
) -> omega_checked_trees::CheckedValueFacts {
    let mut value_roots = omega_core::arena::Arena::default();
    for expression in expressions {
        value_roots.append(omega_checked_trees::CheckedValueFact {
            expression,
            origin: Default::default(),
        });
    }
    omega_checked_trees::CheckedValueFacts::with_roots(value_roots)
}

fn operator_with_spelling(symbol: SymbolHandle, spelling: OperatorSpelling) -> OperatorDefinition {
    OperatorDefinition {
        is_boundary: false,
        symbol,
        name: HandleSpan::empty(),
        type_parameters: HandleSpan::empty(),
        parameters: HandleSpan::empty(),
        return_type: omega_typed_trees::types::TypeReferenceHandle::invalid(),
        contracts: HandleSpan::empty(),
        spelling: Some(spelling),
        token_count: 0,
    }
}
