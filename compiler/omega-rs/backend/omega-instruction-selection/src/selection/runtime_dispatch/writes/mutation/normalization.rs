use crate::InstructionSelectionInput;
use crate::selection::bindings::{
    RuntimeAliasBinding, RuntimeResolvedExpression, resolve_runtime_alias_binding,
};
use crate::selection::storage_places::resolve_runtime_pointee_slot_offset;
use omega_control_flow::StateKey;
use omega_state_values::simplify_state_expression;
use psi_checked_trees::expression::{Expression, ExpressionTable};

pub(in crate::selection::runtime_dispatch::writes) fn simplify_runtime_expression_with_state_locals(
    input: &InstructionSelectionInput<'_>,
    source_key: StateKey,
    statement_index: usize,
    expression: &Expression,
) -> Expression {
    let Some(machine) = input
        .program
        .machines()
        .iter()
        .find(|machine| machine.symbol == source_key.machine)
    else {
        return expression.clone();
    };
    let Some(state) = input
        .program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == source_key.state)
    else {
        return expression.clone();
    };

    simplify_state_expression(input.program, machine, state, statement_index, expression)
}

pub(super) fn normalize_runtime_mutation_expression(
    input: &InstructionSelectionInput<'_>,
    source_key: StateKey,
    statement_index: usize,
    expression: &Expression,
    aliases: &[RuntimeAliasBinding],
    alias_expressions: &ExpressionTable,
) -> RuntimeResolvedExpression {
    let first = resolve_runtime_alias_binding(expression, source_key, aliases, alias_expressions);
    let first_simplified = simplify_runtime_expression_with_state_locals(
        input,
        first.source_key,
        statement_index,
        &first.expression,
    );
    let second = resolve_runtime_alias_binding(
        &first_simplified,
        first.source_key,
        aliases,
        alias_expressions,
    );
    let second_simplified = simplify_runtime_expression_with_state_locals(
        input,
        second.source_key,
        statement_index,
        &second.expression,
    );

    RuntimeResolvedExpression {
        source_key: second.source_key,
        expression: second_simplified,
    }
}

pub(super) fn resolve_runtime_mutation_target(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expression: &Expression,
    aliases: &[RuntimeAliasBinding],
    alias_expressions: &ExpressionTable,
) -> RuntimeResolvedExpression {
    if resolve_runtime_pointee_slot_offset(input, dispatch_index, source_key, expression).is_some()
    {
        return RuntimeResolvedExpression {
            source_key,
            expression: expression.clone(),
        };
    }

    resolve_runtime_alias_binding(expression, source_key, aliases, alias_expressions)
}
