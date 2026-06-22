use super::*;
use omega_control_flow::{
    ControlFlowPlan, StateFlow, StateKey, StateValueFact, StateValueOrigin, StateValueStatementRole,
};
use omega_core::symbols::SymbolHandle;

#[test]
fn copies_control_flow_values_into_abstract_summary() {
    let machine_symbol = SymbolHandle::from_arena_index(1);
    let state_symbol = SymbolHandle::from_arena_index(2);
    let expression = omega_checked_trees::expression::ExpressionHandle::from_arena_index(3);
    let key = StateKey {
        machine: machine_symbol,
        state: state_symbol,
        segment_index: 4,
    };

    let mut control_flow = ControlFlowPlan::default();
    let mut state = StateFlow {
        key,
        ..StateFlow::default()
    };
    control_flow.semantics.values.values.append_to_span(
        &mut state.values.values,
        StateValueFact {
            machine_symbol,
            state_symbol,
            expression,
            origin: StateValueOrigin::Statement {
                statement_index: 5,
                role: StateValueStatementRole::TransitionGuard,
            },
        },
    );
    control_flow.states.insert(state);

    let summary = build_abstract_value_summary(&control_flow);

    assert_eq!(summary.values.len(), 1);
    let copied = summary
        .values
        .iter()
        .next()
        .map(|(_, value)| value)
        .expect("copied value");
    assert_eq!(copied.source_key, key);
    assert_eq!(copied.expression, expression);
    assert_eq!(
        copied.origin,
        AbstractValueOrigin::Statement {
            statement_index: 5,
            role: AbstractValueStatementRole::TransitionGuard,
        }
    );
}
