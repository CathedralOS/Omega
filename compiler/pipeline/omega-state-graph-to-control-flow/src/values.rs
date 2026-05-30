use omega_control_flow::{
    StateValueFact, StateValueOrigin, StateValueStatementRole, StateValueSummary,
};
use omega_core::arena::Arena;
use omega_state_graph::StateGraph;

use crate::handles::remap_value_span;

pub(crate) fn remap_values(state_graph: &StateGraph) -> Arena<StateValueFact> {
    let mut values = Arena::with_capacity(state_graph.semantics.values.values.len());
    for (_, value) in state_graph.semantics.values.values.iter() {
        values.append(remap_value(value));
    }
    values
}

pub(crate) fn remap_value_owned(value: omega_state_graph::StateValueFact) -> StateValueFact {
    StateValueFact {
        machine_symbol: value.machine_symbol,
        state_symbol: value.state_symbol,
        expression: value.expression,
        origin: remap_value_origin(value.origin),
    }
}

pub(crate) fn remap_value_summary(
    summary: &omega_state_graph::StateValueSummary,
) -> StateValueSummary {
    StateValueSummary {
        values: remap_value_span(summary.values),
    }
}

fn remap_value(value: &omega_state_graph::StateValueFact) -> StateValueFact {
    StateValueFact {
        machine_symbol: value.machine_symbol,
        state_symbol: value.state_symbol,
        expression: value.expression,
        origin: remap_value_origin(value.origin),
    }
}

fn remap_value_origin(origin: omega_state_graph::StateValueOrigin) -> StateValueOrigin {
    match origin {
        omega_state_graph::StateValueOrigin::Statement {
            statement_index,
            role,
        } => StateValueOrigin::Statement {
            statement_index,
            role: remap_value_statement_role(role),
        },
    }
}

fn remap_value_statement_role(
    role: omega_state_graph::StateValueStatementRole,
) -> StateValueStatementRole {
    match role {
        omega_state_graph::StateValueStatementRole::Expression => {
            StateValueStatementRole::Expression
        }
        omega_state_graph::StateValueStatementRole::AssignmentTargetSubexpression => {
            StateValueStatementRole::AssignmentTargetSubexpression
        }
        omega_state_graph::StateValueStatementRole::AssignmentValue => {
            StateValueStatementRole::AssignmentValue
        }
        omega_state_graph::StateValueStatementRole::CallArgument => {
            StateValueStatementRole::CallArgument
        }
        omega_state_graph::StateValueStatementRole::LocalInitializer => {
            StateValueStatementRole::LocalInitializer
        }
        omega_state_graph::StateValueStatementRole::TransitionGuard => {
            StateValueStatementRole::TransitionGuard
        }
        omega_state_graph::StateValueStatementRole::TransitionTargetArgument => {
            StateValueStatementRole::TransitionTargetArgument
        }
        omega_state_graph::StateValueStatementRole::TransitionTargetValue => {
            StateValueStatementRole::TransitionTargetValue
        }
    }
}

#[cfg(test)]
mod tests {
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
}
