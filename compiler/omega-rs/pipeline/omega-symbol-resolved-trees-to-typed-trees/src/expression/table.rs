mod lowerer;
mod structural_equality;

use crate::equatable::EqualityScope;
use omega_core::diagnostics::Diagnostic;
use omega_symbol_resolved_trees as resolved;
use omega_typed_trees as typed;

pub(super) fn lower_expression_handle_from_table_with_self_substitution(
    program: Option<&resolved::SymbolResolvedTrees>,
    source: &resolved::expression::ExpressionTable,
    target: &mut typed::expression::ExpressionTable,
    expression: resolved::expression::ExpressionHandle,
    self_substitution: Option<typed::expression::ExpressionHandle>,
) -> Result<typed::expression::ExpressionHandle, Diagnostic> {
    lowerer::ExpressionTableLowerer::new(program, source, target, self_substitution, None)
        .lower(expression)
}

pub(super) fn lower_expression_handle_from_table_in_scope(
    program: &resolved::SymbolResolvedTrees,
    source: &resolved::expression::ExpressionTable,
    target: &mut typed::expression::ExpressionTable,
    expression: resolved::expression::ExpressionHandle,
    scope: Option<&EqualityScope>,
) -> Result<typed::expression::ExpressionHandle, Diagnostic> {
    lowerer::ExpressionTableLowerer::new(Some(program), source, target, None, scope)
        .lower(expression)
}
