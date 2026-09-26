use symbols::SymbolHandle;

use super::super::labels::ContractTargetParameters;

use super::super::direct::{
    direct_context_proves_boolean_expression, direct_context_proves_instantiated_boolean_expression,
};
use super::super::domains::{
    prove_boolean_expression_via_context_domain_membership,
    prove_instantiated_boolean_expression_via_context_domain_membership,
};

pub(super) fn semantic_context_proves_boolean_expression(
    program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    semantic: &crate::fact_plan::FactPlan,
    context: &crate::fact_plan::FactContext,
    expression: symbol_resolved_trees_to_typed_trees::typed_trees::expression::ExpressionHandle,
) -> bool {
    if direct_context_proves_boolean_expression(program, semantic, context, expression) {
        return true;
    }

    match program.expression_table.expression(expression) {
        symbol_resolved_trees_to_typed_trees::typed_trees::expression::ExpressionNode::Borrow(
            inner,
        ) => semantic_context_proves_boolean_expression(program, semantic, context, inner.target),
        symbol_resolved_trees_to_typed_trees::typed_trees::expression::ExpressionNode::Binary(
            binary,
        ) => match binary.operator {
            symbol_resolved_trees_to_typed_trees::typed_trees::expression::BinaryOperator::And => {
                semantic_context_proves_boolean_expression(program, semantic, context, binary.left)
                    && semantic_context_proves_boolean_expression(
                        program,
                        semantic,
                        context,
                        binary.right,
                    )
            }
            symbol_resolved_trees_to_typed_trees::typed_trees::expression::BinaryOperator::Or => {
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
    program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    semantic: &crate::fact_plan::FactPlan,
    context: &crate::fact_plan::FactContext,
    contexts: &[crate::fact_plan::FactContextHandle],
    caller_state_symbol: SymbolHandle,
    statement_index: usize,
    call_site: &crate::semantic::calls::CallSite<'_>,
    target_state: &(impl ContractTargetParameters + ?Sized),
    expression: symbol_resolved_trees_to_typed_trees::typed_trees::expression::ExpressionHandle,
) -> bool {
    if direct_context_proves_instantiated_boolean_expression(
        program,
        semantic,
        context,
        contexts,
        caller_state_symbol,
        statement_index,
        call_site,
        target_state,
        expression,
    ) {
        return true;
    }

    match program.expression_table.expression(expression) {
        symbol_resolved_trees_to_typed_trees::typed_trees::expression::ExpressionNode::Borrow(
            inner,
        ) => semantic_context_proves_instantiated_boolean_expression(
            program,
            semantic,
            context,
            contexts,
            caller_state_symbol,
            statement_index,
            call_site,
            target_state,
            inner.target,
        ),
        symbol_resolved_trees_to_typed_trees::typed_trees::expression::ExpressionNode::Binary(
            binary,
        ) => match binary.operator {
            symbol_resolved_trees_to_typed_trees::typed_trees::expression::BinaryOperator::And => {
                semantic_context_proves_instantiated_boolean_expression(
                    program,
                    semantic,
                    context,
                    contexts,
                    caller_state_symbol,
                    statement_index,
                    call_site,
                    target_state,
                    binary.left,
                ) && semantic_context_proves_instantiated_boolean_expression(
                    program,
                    semantic,
                    context,
                    contexts,
                    caller_state_symbol,
                    statement_index,
                    call_site,
                    target_state,
                    binary.right,
                )
            }
            symbol_resolved_trees_to_typed_trees::typed_trees::expression::BinaryOperator::Or => {
                semantic_context_proves_instantiated_boolean_expression(
                    program,
                    semantic,
                    context,
                    contexts,
                    caller_state_symbol,
                    statement_index,
                    call_site,
                    target_state,
                    binary.left,
                ) || semantic_context_proves_instantiated_boolean_expression(
                    program,
                    semantic,
                    context,
                    contexts,
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
