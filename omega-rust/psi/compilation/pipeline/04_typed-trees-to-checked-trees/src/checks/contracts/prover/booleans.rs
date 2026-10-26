use super::super::direct::{
    InvocationProof, direct_context_proves_boolean_expression,
    direct_context_proves_instantiated_boolean_expression,
};
use super::super::domains::{
    prove_boolean_expression_via_context_domain_membership,
    prove_instantiated_boolean_expression_via_context_domain_membership,
};

pub(super) fn semantic_context_proves_boolean_expression(
    program: &dyn ::validation::CheckingInput,
    semantic: &facts::FactPlan,
    context: &facts::FactContext,
    contexts: &[facts::FactContextHandle],
    point: facts::ProgramPoint,
    expression: typed_trees::expression::ExpressionHandle,
) -> bool {
    if direct_context_proves_boolean_expression(program, semantic, context, expression) {
        return true;
    }

    match program.expression_table.expression(expression) {
        typed_trees::expression::ExpressionNode::Borrow(inner) => {
            semantic_context_proves_boolean_expression(
                program,
                semantic,
                context,
                contexts,
                point,
                inner.target,
            )
        }
        typed_trees::expression::ExpressionNode::Binary(binary) => match binary.operator {
            typed_trees::expression::BinaryOperator::And => {
                semantic_context_proves_boolean_expression(
                    program,
                    semantic,
                    context,
                    contexts,
                    point,
                    binary.left,
                ) && semantic_context_proves_boolean_expression(
                    program,
                    semantic,
                    context,
                    contexts,
                    point,
                    binary.right,
                )
            }
            typed_trees::expression::BinaryOperator::Or => {
                semantic_context_proves_boolean_expression(
                    program,
                    semantic,
                    context,
                    contexts,
                    point,
                    binary.left,
                ) || semantic_context_proves_boolean_expression(
                    program,
                    semantic,
                    context,
                    contexts,
                    point,
                    binary.right,
                )
            }
            _ => prove_boolean_expression_via_context_domain_membership(
                program, semantic, context, contexts, point, expression,
            ),
        },
        _ => prove_boolean_expression_via_context_domain_membership(
            program, semantic, context, contexts, point, expression,
        ),
    }
}

pub(super) fn semantic_context_proves_instantiated_boolean_expression(
    proof: &mut InvocationProof<'_, '_>,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    context: &facts::FactContext,
    expression: typed_trees::expression::ExpressionHandle,
) -> bool {
    if direct_context_proves_instantiated_boolean_expression(
        proof,
        call_frames,
        context,
        expression,
    ) {
        return true;
    }
    let program = proof.scope().program;
    match program.expression_table.expression(expression) {
        typed_trees::expression::ExpressionNode::Borrow(inner) => {
            semantic_context_proves_instantiated_boolean_expression(
                proof,
                call_frames,
                context,
                inner.target,
            )
        }
        typed_trees::expression::ExpressionNode::Binary(binary) => match binary.operator {
            typed_trees::expression::BinaryOperator::And => {
                [binary.left, binary.right].iter().all(|operand| {
                    semantic_context_proves_instantiated_boolean_expression(
                        proof,
                        call_frames,
                        context,
                        *operand,
                    )
                })
            }
            typed_trees::expression::BinaryOperator::Or => {
                [binary.left, binary.right].iter().any(|operand| {
                    semantic_context_proves_instantiated_boolean_expression(
                        proof,
                        call_frames,
                        context,
                        *operand,
                    )
                })
            }
            _ => prove_instantiated_boolean_expression_via_context_domain_membership(
                proof, context, expression,
            ),
        },
        _ => prove_instantiated_boolean_expression_via_context_domain_membership(
            proof, context, expression,
        ),
    }
}
