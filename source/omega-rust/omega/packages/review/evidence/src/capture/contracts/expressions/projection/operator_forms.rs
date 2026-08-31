//! Indexed, binary, and unary operator-expression projection.

use super::super::operators::{
    exact_checked_contract_operator_meaning, project_contract_binary_operator,
    project_contract_unary_operator,
};
use crate::capture::contracts::facts::ContractProjectionContext;
use crate::record::{PackageReviewContractExpression, PackageReviewContractOperatorMeaning};
use omega_compiler::CheckedCompilation;
use psi_diagnostics::Diagnostic;
use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode};

pub(super) fn project_operator_form(
    compilation: &CheckedCompilation,
    context: &ContractProjectionContext<'_>,
    expression: ExpressionHandle,
    node: &ExpressionNode,
    child: &impl Fn(ExpressionHandle) -> Result<PackageReviewContractExpression, Vec<Diagnostic>>,
) -> Option<Result<PackageReviewContractExpression, Vec<Diagnostic>>> {
    match node {
        ExpressionNode::Indexed(indexed) => Some((|| {
            Ok(PackageReviewContractExpression::Indexed {
                meaning: exact_checked_contract_operator_meaning(compilation, context, expression)?,
                collection: Box::new(child(indexed.collection)?),
                index: Box::new(child(indexed.index)?),
            })
        })()),
        ExpressionNode::Binary(binary) => Some((|| {
            Ok(PackageReviewContractExpression::Binary {
                meaning: exact_checked_contract_operator_meaning(compilation, context, expression)?,
                operator: project_contract_binary_operator(binary.operator),
                left: Box::new(child(binary.left)?),
                right: Box::new(child(binary.right)?),
            })
        })()),
        ExpressionNode::Unary(unary) => Some((|| {
            if exact_checked_contract_operator_meaning(compilation, context, expression)?
                != PackageReviewContractOperatorMeaning::Builtin
            {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed {} `{}` unary contract operator is not one closed compiler-owned meaning",
                    context.subject_kind, context.subject_name
                ))]);
            }
            Ok(PackageReviewContractExpression::Unary {
                operator: project_contract_unary_operator(unary.operator),
                operand: Box::new(child(unary.operand)?),
            })
        })()),
        _ => None,
    }
}
