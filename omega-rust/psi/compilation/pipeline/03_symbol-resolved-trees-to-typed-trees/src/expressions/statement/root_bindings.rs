//! Designated product operands become inert call context, not executable AST.
//!
//! The receiver and an optional issued description are ordinary call arguments.
//! Checking and Terminal lowering therefore use the same boundary-call route as
//! other host operations. Only the compiler-owned declaration can carry this
//! context; a similarly named source requirement grants no Build authority.

use crate::lowerer::Lowerer;
use diagnostics::Diagnostic;
use language_semantics::declaration_selection::{
    BuildSelectionRequest, RootBindingOperand, RootBindingRequest,
};

pub(super) fn lower(
    lowerer: &mut Lowerer,
    binding: &symbol_resolved_trees::statement::RootBinding,
) -> Result<typed_trees::statement::StatementNode, Diagnostic> {
    let described = binding.implementation_operand.is_valid();
    let mut arguments = vec![super::arguments::lower_statement_expression(
        lowerer,
        binding.receiver,
    )?];
    let implementation = if described {
        arguments.push(super::arguments::lower_statement_expression(
            lowerer,
            binding.implementation_operand,
        )?);
        RootBindingOperand::Description
    } else {
        RootBindingOperand::AuthoredPath(product_path(&binding.implementation))
    };
    let expression = crate::expressions::build_selections::lower(
        lowerer.source_trees,
        &mut lowerer.typed_trees,
        BuildSelectionRequest::Entry(RootBindingRequest {
            source_span: binding.source_span,
            slot: product_path(&binding.slot),
            implementation,
        }),
        arguments,
        Default::default(),
    )?;
    Ok(typed_trees::statement::StatementNode::Expression(
        expression,
    ))
}

fn product_path(members: &[symbol_resolved_trees::name::DiagnosticName]) -> String {
    let mut path = String::new();
    for member in members {
        if !path.is_empty() {
            path.push_str("::");
        }
        path.push_str(member.as_str());
    }
    path
}
