use omega_checked_trees::{FlowCallFact, FlowStateFact};
use omega_core::symbols::SymbolHandle;
use omega_facts::{FactPayload, FactPlace};

use super::direct::{
    direct_context_proves_boolean_expression, direct_context_proves_instantiated_boolean_expression,
};
use super::domains::{
    prove_boolean_expression_via_context_domain_membership,
    prove_instantiated_boolean_expression_via_context_domain_membership,
};
use super::evaluator::call_site_proves_boolean_contract_expression;

pub(super) fn call_entry_contexts_prove_boolean_contract_expression(
    program: &omega_typed_trees::TypedTrees,
    semantic: &omega_facts::FactPlan,
    state_flow: &FlowStateFact,
    call_flow: &FlowCallFact,
    entry_contexts: &[omega_facts::FactContextHandle],
    expression: omega_typed_trees::expression::ExpressionHandle,
) -> bool {
    let Some(call_site) = crate::find_call_site(
        program,
        state_flow.machine_symbol,
        state_flow.state_symbol,
        call_flow.statement_index,
        call_flow.call_ordinal,
    ) else {
        return false;
    };
    let Some(target_state) = crate::find_state(program, call_flow.target_symbol) else {
        return false;
    };

    entry_contexts.iter().any(|entry_context| {
        let context = semantic.contexts.get(*entry_context);
        semantic_context_proves_instantiated_boolean_expression(
            program,
            semantic,
            context,
            state_flow.state_symbol,
            call_flow.statement_index,
            &call_site,
            target_state,
            expression,
        )
    }) || call_site_proves_boolean_contract_expression(
        program,
        state_flow,
        call_flow,
        &call_site,
        target_state,
        expression,
    )
}

pub(super) fn semantic_contexts_prove_contract_fact(
    program: &omega_typed_trees::TypedTrees,
    semantic: &omega_facts::FactPlan,
    entry_contexts: &[omega_facts::FactContextHandle],
    fact: &omega_facts::Fact,
) -> bool {
    match fact.payload {
        FactPayload::ContractDomainMembership { domain_symbol, .. } => {
            let FactPlace::Place(place) = fact.place else {
                return false;
            };
            entry_contexts.iter().any(|entry_context| {
                let context = semantic.contexts.get(*entry_context);
                semantic
                    .context_view(context)
                    .proves_place_domain_membership_in_program(program, place, domain_symbol)
            })
        }
        FactPayload::ContractBooleanExpression { expression, .. } => {
            matches!(
                program.expression_table.expression(expression),
                omega_checked_trees::expression::ExpressionNode::Boolean(true)
            ) || entry_contexts.iter().any(|entry_context| {
                let context = semantic.contexts.get(*entry_context);
                semantic_context_proves_boolean_expression(program, semantic, context, expression)
            })
        }
        _ => true,
    }
}

fn semantic_context_proves_boolean_expression(
    program: &omega_typed_trees::TypedTrees,
    semantic: &omega_facts::FactPlan,
    context: &omega_facts::FactContext,
    expression: omega_typed_trees::expression::ExpressionHandle,
) -> bool {
    if direct_context_proves_boolean_expression(program, semantic, context, expression) {
        return true;
    }

    match program.expression_table.expression(expression) {
        omega_typed_trees::expression::ExpressionNode::Mutable(inner) => {
            semantic_context_proves_boolean_expression(program, semantic, context, *inner)
        }
        omega_typed_trees::expression::ExpressionNode::Binary(binary) => match binary.operator {
            omega_typed_trees::expression::BinaryOperator::And => {
                semantic_context_proves_boolean_expression(program, semantic, context, binary.left)
                    && semantic_context_proves_boolean_expression(
                        program,
                        semantic,
                        context,
                        binary.right,
                    )
            }
            omega_typed_trees::expression::BinaryOperator::Or => {
                semantic_context_proves_boolean_expression(program, semantic, context, binary.left)
                    || semantic_context_proves_boolean_expression(
                        program,
                        semantic,
                        context,
                        binary.right,
                    )
            }
            _ => prove_boolean_expression_via_context_domain_membership(
                program, semantic, context, expression,
            ),
        },
        _ => prove_boolean_expression_via_context_domain_membership(
            program, semantic, context, expression,
        ),
    }
}

fn semantic_context_proves_instantiated_boolean_expression(
    program: &omega_typed_trees::TypedTrees,
    semantic: &omega_facts::FactPlan,
    context: &omega_facts::FactContext,
    caller_state_symbol: SymbolHandle,
    statement_index: usize,
    call_site: &crate::CallSite<'_>,
    target_state: &omega_typed_trees::state::State,
    expression: omega_typed_trees::expression::ExpressionHandle,
) -> bool {
    if direct_context_proves_instantiated_boolean_expression(
        program,
        semantic,
        context,
        caller_state_symbol,
        statement_index,
        call_site,
        target_state,
        expression,
    ) {
        return true;
    }

    match program.expression_table.expression(expression) {
        omega_typed_trees::expression::ExpressionNode::Mutable(inner) => {
            semantic_context_proves_instantiated_boolean_expression(
                program,
                semantic,
                context,
                caller_state_symbol,
                statement_index,
                call_site,
                target_state,
                *inner,
            )
        }
        omega_typed_trees::expression::ExpressionNode::Binary(binary) => match binary.operator {
            omega_typed_trees::expression::BinaryOperator::And => {
                semantic_context_proves_instantiated_boolean_expression(
                    program,
                    semantic,
                    context,
                    caller_state_symbol,
                    statement_index,
                    call_site,
                    target_state,
                    binary.left,
                ) && semantic_context_proves_instantiated_boolean_expression(
                    program,
                    semantic,
                    context,
                    caller_state_symbol,
                    statement_index,
                    call_site,
                    target_state,
                    binary.right,
                )
            }
            omega_typed_trees::expression::BinaryOperator::Or => {
                semantic_context_proves_instantiated_boolean_expression(
                    program,
                    semantic,
                    context,
                    caller_state_symbol,
                    statement_index,
                    call_site,
                    target_state,
                    binary.left,
                ) || semantic_context_proves_instantiated_boolean_expression(
                    program,
                    semantic,
                    context,
                    caller_state_symbol,
                    statement_index,
                    call_site,
                    target_state,
                    binary.right,
                )
            }
            _ => prove_instantiated_boolean_expression_via_context_domain_membership(
                program,
                semantic,
                context,
                caller_state_symbol,
                statement_index,
                call_site,
                target_state,
                expression,
            ),
        },
        _ => prove_instantiated_boolean_expression_via_context_domain_membership(
            program,
            semantic,
            context,
            caller_state_symbol,
            statement_index,
            call_site,
            target_state,
            expression,
        ),
    }
}
