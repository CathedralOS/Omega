use crate::expression::lower_expression_handle_from_table_with_self_substitution;
use crate::name::lower_name;
use omega_core::diagnostics::Diagnostic;
use omega_symbol_resolved_trees as resolved;
use omega_typed_trees as typed;

pub(super) fn lower_expression_handle_span_from_table(
    program: Option<&resolved::SymbolResolvedTrees>,
    source: &resolved::expression::ExpressionTable,
    target: &mut typed::expression::ExpressionTable,
    expressions: omega_core::arena::HandleSpan<resolved::expression::ExpressionHandle>,
    self_substitution: Option<typed::expression::ExpressionHandle>,
) -> Result<omega_core::arena::HandleSpan<typed::expression::ExpressionHandle>, Diagnostic> {
    let lowered = source
        .expression_handles(expressions)
        .iter()
        .copied()
        .map(|expression| {
            lower_expression_handle_from_table_with_self_substitution(
                program,
                source,
                target,
                expression,
                self_substitution,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(target.insert_expression_handles(lowered))
}

pub(super) fn lower_struct_literal_field_span_from_table(
    program: Option<&resolved::SymbolResolvedTrees>,
    source: &resolved::expression::ExpressionTable,
    target: &mut typed::expression::ExpressionTable,
    fields: omega_core::arena::HandleSpan<resolved::expression::TableStructLiteralField>,
    self_substitution: Option<typed::expression::ExpressionHandle>,
) -> Result<omega_core::arena::HandleSpan<typed::expression::TableStructLiteralField>, Diagnostic> {
    let lowered = source
        .struct_fields(fields)
        .iter()
        .map(|field| {
            let value = lower_expression_handle_from_table_with_self_substitution(
                program,
                source,
                target,
                field.value,
                self_substitution,
            )?;
            Ok(typed::expression::TableStructLiteralField {
                name: lower_name(&field.name),
                value,
            })
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;

    Ok(target.insert_struct_fields(lowered))
}
