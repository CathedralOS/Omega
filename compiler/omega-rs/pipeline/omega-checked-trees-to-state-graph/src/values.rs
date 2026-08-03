use omega_state_graph::{
    StateGraph, StateKey, StateValueFact, StateValueOrigin, StateValueStatementRole,
    StateValueSummary,
};
use psi_checked_trees::{CheckedTrees, CheckedValueOrigin, CheckedValueStatementRole};
use psi_numerics::arithmetic::ArithmeticPolicyAdapter;

pub(crate) fn state_value_summary(
    state_graph: &mut StateGraph,
    program: &CheckedTrees,
    key: StateKey,
) -> StateValueSummary {
    let mut state_values = Vec::new();

    for (_, value) in program.facts.values.values.iter() {
        let Some((machine_symbol, state_symbol, statement_index, role)) =
            statement_origin(value.origin)
        else {
            continue;
        };

        if machine_symbol != key.machine || state_symbol != key.state {
            continue;
        }

        append_or_enrich_value(
            &mut state_values,
            StateValueFact {
                machine_symbol,
                state_symbol,
                expression: value.expression,
                origin: StateValueOrigin::Statement {
                    statement_index,
                    role: remap_value_statement_role(role),
                },
                arithmetic_policy_adapter: program
                    .facts
                    .operators
                    .policy_adapter_evidence_for_expression_in_origin(
                        value.expression,
                        value.origin,
                    ),
                operator_provider_plan_identity: program
                    .facts
                    .operators
                    .provider_plan_identity_for_expression_in_origin(
                        value.expression,
                        value.origin,
                    ),
            },
        );
    }

    // Operator collection walks every root expression recursively while
    // retaining the root's statement origin. Preserve those nested operator
    // expressions too: a policy adapter applies at each arithmetic node, not
    // merely at the statement's outermost value.
    for (_, operator_use) in program.facts.operators.uses.iter() {
        append_operator_value(
            &mut state_values,
            key,
            operator_use.expression,
            operator_use.origin,
            operator_use.policy_adapter,
            (operator_use.provider_plan_identity != 0)
                .then_some(operator_use.provider_plan_identity),
        );
    }
    for (_, operator_use) in program.facts.operators.named_uses.iter() {
        append_operator_value(
            &mut state_values,
            key,
            operator_use.expression,
            operator_use.origin,
            operator_use.policy_adapter,
            (operator_use.provider_plan_identity != 0)
                .then_some(operator_use.provider_plan_identity),
        );
    }

    StateValueSummary {
        values: state_graph
            .semantics
            .values
            .values
            .insert_many(state_values),
    }
}

fn append_operator_value(
    values: &mut Vec<StateValueFact>,
    key: StateKey,
    expression: psi_checked_trees::expression::ExpressionHandle,
    origin: CheckedValueOrigin,
    arithmetic_policy_adapter: ArithmeticPolicyAdapter,
    operator_provider_plan_identity: Option<u64>,
) {
    let Some((machine_symbol, state_symbol, statement_index, role)) = statement_origin(origin)
    else {
        return;
    };
    if machine_symbol != key.machine || state_symbol != key.state {
        return;
    }
    append_or_enrich_value(
        values,
        StateValueFact {
            machine_symbol,
            state_symbol,
            expression,
            origin: StateValueOrigin::Statement {
                statement_index,
                role: remap_value_statement_role(role),
            },
            arithmetic_policy_adapter: Some(arithmetic_policy_adapter),
            operator_provider_plan_identity,
        },
    );
}

fn append_or_enrich_value(values: &mut Vec<StateValueFact>, value: StateValueFact) {
    if let Some(index) = values.iter().position(|existing| {
        existing.machine_symbol == value.machine_symbol
            && existing.state_symbol == value.state_symbol
            && existing.expression == value.expression
            && existing.origin == value.origin
    }) {
        match (
            values[index].operator_provider_plan_identity,
            value.operator_provider_plan_identity,
        ) {
            (None, Some(identity)) => {
                values[index].operator_provider_plan_identity = Some(identity)
            }
            (Some(left), Some(right)) if left != right => {
                values.push(value);
                return;
            }
            _ => {}
        }
        match (
            values[index].arithmetic_policy_adapter,
            value.arithmetic_policy_adapter,
        ) {
            (None, Some(adapter)) => values[index].arithmetic_policy_adapter = Some(adapter),
            (Some(left), Some(right)) if left != right => {
                // Retain the contradiction as a second fact. Downstream lookup
                // rejects conflicting carried evidence instead of choosing one.
                values.push(value);
            }
            _ => {}
        }
        return;
    }
    values.push(value);
}

fn statement_origin(
    origin: CheckedValueOrigin,
) -> Option<(
    psi_symbols::SymbolHandle,
    psi_symbols::SymbolHandle,
    usize,
    CheckedValueStatementRole,
)> {
    let CheckedValueOrigin::StateStatement {
        machine_symbol,
        state_symbol,
        statement_index,
        role,
    } = origin
    else {
        return None;
    };
    Some((machine_symbol, state_symbol, statement_index, role))
}

pub(crate) fn remap_state_value_summary(
    target: &mut StateGraph,
    source_values: &psi_arena::Arena<StateValueFact>,
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
mod tests;
