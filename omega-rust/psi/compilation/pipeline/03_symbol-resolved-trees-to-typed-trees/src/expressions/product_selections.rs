//! Retain designated Product paths without demanding host type formation.
//! The namespace is exact toolchain data; ordinary same-named calls keep their
//! normal path. Final Product admission owns declaration visibility and roles.

use diagnostics::Diagnostic;
use language_semantics::declaration_selection::{
    BuildOperation, BuildSelectionRequest, ProductDeclarationOperand, ProductDeclarationRequest,
};
use symbols::JoinedPath;

pub(crate) fn request(
    source: &symbol_resolved_trees::SymbolResolvedTrees,
    namespace: symbols::SymbolHandle,
    target: symbols::SymbolHandle,
    name: &str,
    operands: &[symbol_resolved_trees::expression::StaticMachineArgument],
    ordinary_count: usize,
    evidence_count: usize,
    span: source::SourceSpan,
) -> Result<Option<BuildSelectionRequest>, Diagnostic> {
    let Some(
        operation @ (BuildOperation::ProviderSelection | BuildOperation::RepresentationSelection),
    ) = BuildOperation::from_call_target(name)
    else {
        return Ok(None);
    };
    if target.is_valid()
        || !namespace.is_valid()
        || !source.symbols.is_build_prelude_symbol(namespace)
        || !source
            .data_definitions
            .iter()
            .any(|data| data.symbol == namespace && data.name.as_str() == "Build")
    {
        return Ok(None);
    }
    if operands.len() != 2
        || evidence_count != 0
        || !(ordinary_count == 1
            || (operation == BuildOperation::ProviderSelection && ordinary_count == 2))
    {
        return Err(Diagnostic::error(format!(
            "{} requires two bare declaration operands and its explicit mutable Build argument",
            operation.authored_spelling()
        ))
        .with_source_span(span));
    }
    let operands = operands.iter().map(|operand| {
        if operand.type_reference.is_valid() || operand.application.is_some()
            || operand.const_literal.is_some() || operand.evidence_projection.is_some()
            || operand.path.is_empty() {
            return Err(Diagnostic::error("Build selection operands must name bare declarations, not applied types, conformance applications, values or projections")
                .with_source_span(span));
        }
        let first = operand.path.first().expect("nonempty path checked").source_span();
        let last = operand.path.last().expect("nonempty path checked").source_span();
        if first.source_id != last.source_id || first.span.start >= last.span.end {
            return Err(Diagnostic::error("Product operand has no single authored source coordinate")
                .with_source_span(span));
        }
        Ok(ProductDeclarationOperand {
            path: operand.path.joined_path(),
            source_span: source::SourceSpan::new(first.source_id,
                source::Span::new(first.span.start, last.span.end)),
        })
    }).collect::<Result<Vec<_>, Diagnostic>>()?.into_boxed_slice();
    Ok(Some(BuildSelectionRequest::Declaration(
        std::sync::Arc::new(ProductDeclarationRequest {
            source_span: span,
            operation,
            operands,
        }),
    )))
}
