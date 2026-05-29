use crate::lowerer::Lowerer;
use omega_core::diagnostics::Diagnostic;
use omega_symbol_resolved_trees as resolved;
use omega_typed_trees as typed;

mod domain_membership;
mod name_paths;
mod operators;
mod spans;
mod table;
#[cfg(test)]
mod tests;

pub(crate) fn lower_expression_handle(
    lowerer: &mut Lowerer,
    expression: resolved::expression::ExpressionHandle,
) -> Result<typed::expression::ExpressionHandle, Diagnostic> {
    let source = &lowerer.source_trees.tables.bodies.expressions;
    lower_expression_handle_from_table_with_self_substitution(
        Some(lowerer.source_trees),
        source,
        &mut lowerer.typed_trees.expression_table,
        expression,
        None,
    )
}

pub(crate) fn lower_expression_handle_from_table(
    source: &resolved::expression::ExpressionTable,
    target: &mut typed::expression::ExpressionTable,
    expression: resolved::expression::ExpressionHandle,
) -> Result<typed::expression::ExpressionHandle, Diagnostic> {
    lower_expression_handle_from_table_with_self_substitution(
        None, source, target, expression, None,
    )
}

pub(crate) fn lower_expression_handle_from_table_in_program(
    program: &resolved::SymbolResolvedTrees,
    source: &resolved::expression::ExpressionTable,
    target: &mut typed::expression::ExpressionTable,
    expression: resolved::expression::ExpressionHandle,
) -> Result<typed::expression::ExpressionHandle, Diagnostic> {
    lower_expression_handle_from_table_with_self_substitution(
        Some(program),
        source,
        target,
        expression,
        None,
    )
}

pub(super) fn lower_expression_handle_from_table_with_self_substitution(
    program: Option<&resolved::SymbolResolvedTrees>,
    source: &resolved::expression::ExpressionTable,
    target: &mut typed::expression::ExpressionTable,
    expression: resolved::expression::ExpressionHandle,
    self_substitution: Option<typed::expression::ExpressionHandle>,
) -> Result<typed::expression::ExpressionHandle, Diagnostic> {
    table::lower_expression_handle_from_table_with_self_substitution(
        program,
        source,
        target,
        expression,
        self_substitution,
    )
}
