use omega_abstract_operations::{
    AbstractValueFact, AbstractValueOrigin, AbstractValueStatementRole, AbstractValueSummary,
};
use omega_control_flow::{ControlFlowPlan, StateValueOrigin, StateValueStatementRole};

pub(super) fn build_abstract_value_summary(control_flow: &ControlFlowPlan) -> AbstractValueSummary {
    let mut summary = AbstractValueSummary::with_capacity(control_flow.values.len());

    for (_, state) in control_flow.states.iter() {
        for value in control_flow.values.span_or_empty(state.values.values) {
            summary.values.insert(AbstractValueFact {
                source_key: state.key,
                machine_symbol: value.machine_symbol,
                state_symbol: value.state_symbol,
                expression: value.expression,
                origin: remap_value_origin(value.origin),
            });
        }
    }

    summary
}

fn remap_value_origin(origin: StateValueOrigin) -> AbstractValueOrigin {
    match origin {
        StateValueOrigin::Statement {
            statement_index,
            role,
        } => AbstractValueOrigin::Statement {
            statement_index,
            role: remap_value_statement_role(role),
        },
    }
}

fn remap_value_statement_role(role: StateValueStatementRole) -> AbstractValueStatementRole {
    match role {
        StateValueStatementRole::Expression => AbstractValueStatementRole::Expression,
        StateValueStatementRole::AssignmentTargetSubexpression => {
            AbstractValueStatementRole::AssignmentTargetSubexpression
        }
        StateValueStatementRole::AssignmentValue => AbstractValueStatementRole::AssignmentValue,
        StateValueStatementRole::CallArgument => AbstractValueStatementRole::CallArgument,
        StateValueStatementRole::LocalInitializer => AbstractValueStatementRole::LocalInitializer,
        StateValueStatementRole::TransitionGuard => AbstractValueStatementRole::TransitionGuard,
        StateValueStatementRole::TransitionTargetArgument => {
            AbstractValueStatementRole::TransitionTargetArgument
        }
        StateValueStatementRole::TransitionTargetValue => {
            AbstractValueStatementRole::TransitionTargetValue
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_control_flow::{
        ControlFlowPlan, StateFlow, StateKey, StateValueFact, StateValueOrigin,
        StateValueStatementRole,
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
        control_flow.values.append_to_span(
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
}
