use omega_control_flow::{
    StateValueFact, StateValueOrigin, StateValueStatementRole, StateValueSummary,
};
use omega_state_graph::StateGraph;
use psi_arena::Arena;

use crate::arena_remap::remap_arena;
use crate::handles::remap_value_span;

pub(crate) fn remap_values(state_graph: &StateGraph) -> Arena<StateValueFact> {
    remap_arena(&state_graph.semantics.values.values, remap_value_owned)
}

pub(crate) fn remap_value_owned(value: omega_state_graph::StateValueFact) -> StateValueFact {
    StateValueFact {
        machine_symbol: value.machine_symbol,
        state_symbol: value.state_symbol,
        expression: value.expression,
        origin: remap_value_origin(value.origin),
        arithmetic_policy_adapter: value.arithmetic_policy_adapter,
        operator_provider_plan_identity: value.operator_provider_plan_identity,
    }
}

pub(crate) fn remap_value_summary(
    summary: &omega_state_graph::StateValueSummary,
) -> StateValueSummary {
    StateValueSummary {
        values: remap_value_span(summary.values),
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
mod tests;
