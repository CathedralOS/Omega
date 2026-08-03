use super::*;
use omega_control_flow::{
    ControlFlowPlan, StateFlow, StateKey, StateValueFact, StateValueOrigin, StateValueStatementRole,
};
use psi_symbols::SymbolHandle;

#[test]
fn copies_control_flow_values_into_abstract_summary() {
    let machine_symbol = SymbolHandle::from_arena_index(1);
    let state_symbol = SymbolHandle::from_arena_index(2);
    let expression = psi_checked_trees::expression::ExpressionHandle::from_arena_index(3);
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
            arithmetic_policy_adapter: Some(
                psi_numerics::arithmetic::ArithmeticPolicyAdapter::FloatSaturatingOverflowOnly {
                    format: psi_numerics::float_semantics::FloatFormat::BINARY32,
                },
            ),
            operator_provider_plan_identity: Some(0x1234_5678_9abc_def0),
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
    assert_eq!(
        copied.arithmetic_policy_adapter,
        Some(
            psi_numerics::arithmetic::ArithmeticPolicyAdapter::FloatSaturatingOverflowOnly {
                format: psi_numerics::float_semantics::FloatFormat::BINARY32,
            }
        )
    );
    assert_eq!(
        copied.operator_provider_plan_identity,
        Some(0x1234_5678_9abc_def0)
    );
}
