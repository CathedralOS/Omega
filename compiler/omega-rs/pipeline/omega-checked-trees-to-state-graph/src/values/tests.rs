use super::*;
use psi_checked_trees::CheckedValueFact;
use psi_checked_trees::expression::ExpressionHandle;
use psi_symbols::SymbolHandle;

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
        ..CheckedValueFact::default()
    });
    program.facts.values.values.insert(CheckedValueFact {
        expression,
        origin: CheckedValueOrigin::StateStatement {
            machine_symbol,
            state_symbol: other_state_symbol,
            statement_index: 4,
            role: CheckedValueStatementRole::CallArgument,
        },
        ..CheckedValueFact::default()
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
    assert_eq!(values[0].arithmetic_policy_adapter, None);
    assert_eq!(values[0].operator_provider_plan_identity, None);
}

#[test]
fn state_value_summary_carries_nested_checked_policy_adapter_evidence() {
    let machine_symbol = SymbolHandle::from_arena_index(31);
    let state_symbol = SymbolHandle::from_arena_index(32);
    let root_expression = ExpressionHandle::from_arena_index(41);
    let nested_expression = ExpressionHandle::from_arena_index(42);
    let origin = CheckedValueOrigin::StateStatement {
        machine_symbol,
        state_symbol,
        statement_index: 5,
        role: CheckedValueStatementRole::LocalInitializer,
    };

    let mut program = CheckedTrees::default();
    program.facts.values.values.insert(CheckedValueFact {
        expression: root_expression,
        origin,
        ..CheckedValueFact::default()
    });
    program
        .facts
        .operators
        .uses
        .insert(psi_checked_trees::CheckedOperatorUseFact {
            expression: nested_expression,
            origin,
            provider_plan_identity: 0x1234_5678_9abc_def0,
            policy_adapter:
                psi_checked_trees::CheckedArithmeticPolicyAdapter::FloatTrappingNonFinite {
                    format: psi_numerics::float_semantics::FloatFormat::BINARY64,
                },
            ..psi_checked_trees::CheckedOperatorUseFact::default()
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
    let nested = values
        .iter()
        .find(|value| value.expression == nested_expression)
        .expect("nested operator value");

    assert_eq!(
        nested.arithmetic_policy_adapter,
        Some(
            psi_numerics::arithmetic::ArithmeticPolicyAdapter::FloatTrappingNonFinite {
                format: psi_numerics::float_semantics::FloatFormat::BINARY64,
            }
        )
    );
    assert_eq!(
        nested.operator_provider_plan_identity,
        Some(0x1234_5678_9abc_def0)
    );
}
