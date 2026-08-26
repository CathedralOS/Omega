use omega_abstract_operations::{
    AbstractValueFact, AbstractValueOrigin, AbstractValueStatementRole, AbstractValueSummary,
};
use omega_control_flow::{ControlFlowPlan, StateValueOrigin, StateValueStatementRole};

pub(super) fn build_abstract_value_summary(control_flow: &ControlFlowPlan) -> AbstractValueSummary {
    let mut summary =
        AbstractValueSummary::with_capacity(control_flow.semantics.values.values.len());

    for (_, state) in control_flow.states.iter() {
        for value in control_flow
            .semantics
            .values
            .values
            .span_or_empty(state.values.values)
        {
            summary.values.insert(AbstractValueFact {
                source_key: state.key,
                machine_symbol: value.machine_symbol,
                state_symbol: value.state_symbol,
                expression: value.expression,
                origin: remap_value_origin(value.origin),
                arithmetic_policy_adapter: value.arithmetic_policy_adapter,
                operator_provider_plan_identity: value.operator_provider_plan_identity,
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
mod tests;
