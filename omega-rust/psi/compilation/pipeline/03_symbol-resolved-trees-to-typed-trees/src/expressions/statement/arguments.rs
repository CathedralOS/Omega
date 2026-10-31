use crate::expressions::expression::lower_expression_handle;
use crate::lowerer::Lowerer;
use crate::tables::resolved_statements;
use crate::tables::statement_table_mut;
use arena::HandleSpan;
use diagnostics::Diagnostic;
use symbol_resolved_trees as resolved;
use typed_trees as typed;

pub(super) fn lower_statement_expression(
    lowerer: &mut Lowerer,
    expression: resolved::expression::ExpressionHandle,
) -> Result<typed::expression::ExpressionHandle, Diagnostic> {
    lower_expression_handle(lowerer, expression)
}

pub(super) fn lower_statement_argument_span(
    lowerer: &mut Lowerer,
    arguments: HandleSpan<resolved::expression::ExpressionHandle>,
) -> Result<HandleSpan<typed::expression::ExpressionHandle>, Diagnostic> {
    let arguments = resolved_statements(lowerer.source_trees)
        .expression_handles(arguments)
        .to_vec();
    let lowered_arguments = arguments
        .into_iter()
        .map(|argument| lower_statement_expression(lowerer, argument))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(statement_table_mut(&mut lowerer.typed_trees).insert_expression_handles(lowered_arguments))
}

pub(super) fn lower_statement_path_members(
    lowerer: &mut Lowerer,
    path: HandleSpan<resolved::name::DiagnosticName>,
) -> HandleSpan<typed::name::Identifier> {
    let mut lowered_path = HandleSpan::empty();

    for member in resolved_statements(lowerer.source_trees).name_path_members(path) {
        statement_table_mut(&mut lowerer.typed_trees)
            .push_name_path_member(&mut lowered_path, crate::lowerer::name::lower_name(member));
    }

    lowered_path
}
