use super::*;
use psi_checked_trees::expression::ExpressionHandle;
use psi_symbols::SymbolHandle;

#[test]
fn remap_value_summary_preserves_statement_value_handles() {
    let value = omega_state_graph::StateValueFact {
        machine_symbol: SymbolHandle::from_arena_index(1),
        state_symbol: SymbolHandle::from_arena_index(2),
        expression: ExpressionHandle::from_arena_index(3),
        origin: omega_state_graph::StateValueOrigin::Statement {
            statement_index: 4,
            role: omega_state_graph::StateValueStatementRole::CallArgument,
        },
        arithmetic_policy_adapter: Some(
            psi_numerics::arithmetic::ArithmeticPolicyAdapter::FloatTrappingNonFinite {
                format: psi_numerics::float_semantics::FloatFormat::BINARY64,
            },
        ),
        operator_provider_plan_identity: Some(0x1234_5678_9abc_def0),
    };
    let mut values = Arena::new();
    let mut span = psi_arena::HandleSpan::empty();
    values.append_to_span(&mut span, value);

    let summary = remap_value_summary(&omega_state_graph::StateValueSummary { values: span });

    assert_eq!(summary.values.count(), 1);
    assert_eq!(
        summary.values.start().arena_index(),
        span.start().arena_index()
    );
    let copied = remap_value_owned(
        values
            .iter()
            .next()
            .map(|(_, value)| value.clone())
            .expect("source value"),
    );
    assert_eq!(
        copied.arithmetic_policy_adapter,
        Some(
            psi_numerics::arithmetic::ArithmeticPolicyAdapter::FloatTrappingNonFinite {
                format: psi_numerics::float_semantics::FloatFormat::BINARY64,
            }
        )
    );
    assert_eq!(
        copied.operator_provider_plan_identity,
        Some(0x1234_5678_9abc_def0)
    );
}
