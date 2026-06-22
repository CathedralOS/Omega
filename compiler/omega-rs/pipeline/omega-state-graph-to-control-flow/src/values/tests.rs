use super::*;
use omega_checked_trees::expression::ExpressionHandle;
use omega_core::symbols::SymbolHandle;

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
    };
    let mut values = Arena::new();
    let mut span = omega_core::arena::HandleSpan::empty();
    values.append_to_span(&mut span, value);

    let summary = remap_value_summary(&omega_state_graph::StateValueSummary { values: span });

    assert_eq!(summary.values.count(), 1);
    assert_eq!(
        summary.values.start().arena_index(),
        span.start().arena_index()
    );
}
