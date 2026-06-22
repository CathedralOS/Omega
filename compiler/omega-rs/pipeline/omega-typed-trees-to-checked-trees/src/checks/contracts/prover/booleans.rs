use omega_core::symbols::SymbolHandle;

use super::super::direct::{
    direct_context_proves_boolean_expression, direct_context_proves_instantiated_boolean_expression,
};
use super::super::domains::{
    prove_boolean_expression_via_context_domain_membership,
    prove_instantiated_boolean_expression_via_context_domain_membership,
};

pub(super) fn semantic_context_proves_boolean_expression(
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

pub(super) fn semantic_context_proves_instantiated_boolean_expression(
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
