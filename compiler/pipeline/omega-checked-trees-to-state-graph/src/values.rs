use omega_checked_trees::{CheckedTrees, CheckedValueOrigin, CheckedValueStatementRole};
use omega_state_graph::{
    StateGraph, StateKey, StateValueFact, StateValueOrigin, StateValueStatementRole,
    StateValueSummary,
};

pub(crate) fn state_value_summary(
    state_graph: &mut StateGraph,
    program: &CheckedTrees,
    key: StateKey,
) -> StateValueSummary {
    let mut values = omega_core::arena::HandleSpan::empty();

    for (_, value) in program.facts.values.values.iter() {
        let CheckedValueOrigin::StateStatement {
            machine_symbol,
            state_symbol,
            statement_index,
            role,
        } = value.origin
        else {
            continue;
        };

        if machine_symbol != key.machine || state_symbol != key.state {
            continue;
        }

        state_graph.semantics.values.values.append_to_span(
            &mut values,
            StateValueFact {
                machine_symbol,
                state_symbol,
                expression: value.expression,
                origin: StateValueOrigin::Statement {
                    statement_index,
                    role: remap_value_statement_role(role),
                },
            },
        );
    }

    StateValueSummary { values }
}

pub(crate) fn remap_state_value_summary(
    target: &mut StateGraph,
    source_values: &omega_core::arena::Arena<StateValueFact>,
    values: &StateValueSummary,
) -> StateValueSummary {
    StateValueSummary {
        values: target
            .semantics
            .values
            .values
            .insert_many(source_values.span_or_empty(values.values).iter().cloned()),
    }
}

fn remap_value_statement_role(role: CheckedValueStatementRole) -> StateValueStatementRole {
    match role {
        CheckedValueStatementRole::Expression => StateValueStatementRole::Expression,
        CheckedValueStatementRole::AssignmentTargetSubexpression => {
            StateValueStatementRole::AssignmentTargetSubexpression
        }
        CheckedValueStatementRole::AssignmentValue => StateValueStatementRole::AssignmentValue,
        CheckedValueStatementRole::CallArgument => StateValueStatementRole::CallArgument,
        CheckedValueStatementRole::LocalInitializer => StateValueStatementRole::LocalInitializer,
        CheckedValueStatementRole::TransitionGuard => StateValueStatementRole::TransitionGuard,
        CheckedValueStatementRole::TransitionTargetArgument => {
            StateValueStatementRole::TransitionTargetArgument
        }
        CheckedValueStatementRole::TransitionTargetValue => {
            StateValueStatementRole::TransitionTargetValue
        }
    }
}

#[cfg(test)]
mod tests {
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
}
