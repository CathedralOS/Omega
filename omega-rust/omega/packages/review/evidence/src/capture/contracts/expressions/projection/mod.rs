//! Recursive contract-expression projection by semantic expression family.
//!
//! This entrance retains recursion depth, substitutions, and checked-fact
//! custody. Value forms, operator forms, calls, and members descend into named
//! leaves; names and casts reuse their sibling semantic projectors.

mod call_expression;
mod member_expression;
mod operator_forms;
mod value_forms;

#[cfg(test)]
mod lifetime_tests;

use super::casts::project_contract_cast;
use super::names::project_contract_name_expression;
use crate::capture::contracts::facts::ContractProjectionContext;
use crate::record::PackageReviewContractExpression;
use omega_compiler::CheckedCompilation;
use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;

pub(crate) fn project_contract_expression(
    compilation: &CheckedCompilation,
    context: &ContractProjectionContext<'_>,
    binders: &[(SymbolHandle, String)],
    expression: psi_typed_trees::expression::ExpressionHandle,
    checked_fact: Option<psi_arena::Handle<psi_typed_trees::domain::ProofFact>>,
    depth: usize,
) -> Result<PackageReviewContractExpression, Vec<Diagnostic>> {
    project_contract_expression_with_substitutions(
        compilation,
        context,
        binders,
        expression,
        &[],
        &[],
        checked_fact,
        depth,
    )
}

pub(crate) fn project_contract_expression_with_substitutions(
    compilation: &CheckedCompilation,
    context: &ContractProjectionContext<'_>,
    binders: &[(SymbolHandle, String)],
    expression: psi_typed_trees::expression::ExpressionHandle,
    substitutions: &[(SymbolHandle, PackageReviewContractExpression)],
    projection_substitutions: &[(SymbolHandle, psi_typed_trees::expression::ExpressionHandle)],
    checked_fact: Option<psi_arena::Handle<psi_typed_trees::domain::ProofFact>>,
    depth: usize,
) -> Result<PackageReviewContractExpression, Vec<Diagnostic>> {
    use psi_typed_trees::expression::ExpressionNode;

    if depth >= 256 {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` contract expression exceeds the package-review depth limit",
            context.subject_kind, context.subject_name
        ))]);
    }
    let child = |expression| {
        project_contract_expression_with_substitutions(
            compilation,
            context,
            binders,
            expression,
            substitutions,
            projection_substitutions,
            checked_fact,
            depth + 1,
        )
    };
    let node = compilation.expression_table.expression(expression);
    match node {
        ExpressionNode::Boolean(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::ArrayLiteral(_)
        | ExpressionNode::StructLiteral(_)
        | ExpressionNode::Atomic(_)
        | ExpressionNode::Range(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_)
        | ExpressionNode::Borrow(_) => {
            value_forms::project_value_form(compilation, context, binders, node, &child)
                .expect("routed value form")
        }
        ExpressionNode::Indexed(_) | ExpressionNode::Binary(_) | ExpressionNode::Unary(_) => {
            operator_forms::project_operator_form(compilation, context, expression, node, &child)
                .expect("routed operator form")
        }
        ExpressionNode::Call(call) => call_expression::project_call_expression(
            compilation,
            context,
            binders,
            expression,
            call,
            checked_fact,
            &child,
        ),
        ExpressionNode::Name(path) => project_contract_name_expression(
            compilation,
            context,
            binders,
            expression,
            path,
            substitutions,
            checked_fact,
        ),
        ExpressionNode::Member(member) => member_expression::project_member_expression(
            compilation,
            context,
            expression,
            member,
            substitutions,
            projection_substitutions,
            checked_fact,
            &child,
        ),
        ExpressionNode::Cast(cast) => {
            project_contract_cast(compilation, context, binders, cast, child)
        }
    }
}
