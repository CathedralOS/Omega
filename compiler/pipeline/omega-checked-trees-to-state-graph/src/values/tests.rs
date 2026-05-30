use super::*;
use omega_checked_trees::CheckedValueFact;
use omega_checked_trees::expression::ExpressionHandle;
use omega_core::symbols::SymbolHandle;

#[test]
fn state_value_summary_keeps_values_for_matching_state() {
    let machine_symbol = SymbolHandle::from_arena_index(11);
    let state_symbol = SymbolHandle::from_arena_index(12);
    let other_state_symbol = SymbolHandle::from_arena_index(13);
    let expression = ExpressionHandle::from_arena_index(21);

    let mut program = CheckedTrees::default();
    program.facts.values.values.insert(CheckedValueFact {
        expression,
        origin: CheckedValueOrigin::StateStatement {
            machine_symbol,
            state_symbol,
            statement_index: 3,
            role: CheckedValueStatementRole::AssignmentValue,
        },
    });
    program.facts.values.values.insert(CheckedValueFact {
        expression,
        origin: CheckedValueOrigin::StateStatement {
            machine_symbol,
            state_symbol: other_state_symbol,
            statement_index: 4,
            role: CheckedValueStatementRole::CallArgument,
        },
    });

    let mut state_graph = StateGraph::default();
    let summary = state_value_summary(
        &mut state_graph,
        &program,
        StateKey {
            machine: machine_symbol,
            state: state_symbol,
            segment_index: 0,
        },
    );

    let values = state_graph
        .semantics
        .values
        .values
        .span_or_empty(summary.values);
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].expression, expression);
    assert_eq!(
        values[0].origin,
        StateValueOrigin::Statement {
            statement_index: 3,
            role: StateValueStatementRole::AssignmentValue,
        }
    );
}
