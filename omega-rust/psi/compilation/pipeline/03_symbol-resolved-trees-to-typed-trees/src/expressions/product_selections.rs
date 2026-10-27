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
    // Resolved enum operations are exact prelude declarations, unlike the
    // unresolved syntax carrying designated Product declaration operands.
    if target.is_valid() && source.symbols.is_build_prelude_symbol(target) {
        use language_semantics::declaration_selection::{
            EnumExclusionRequest, ExclusionVocabulary,
        };
        let vocabulary = match name {
            "exclude_crash" => Some(ExclusionVocabulary::CrashCause),
            "exclude_physical_authority" => Some(ExclusionVocabulary::PhysicalAuthorityClass),
            _ => None,
        };
        if let Some(vocabulary) = vocabulary {
            if ordinary_count != 2 || evidence_count != 0 || !operands.is_empty() {
                return Err(Diagnostic::error(
                    "behavior exclusion requires its mutable Build and one evaluated enum value",
                )
                .with_source_span(span));
            }
            return Ok(Some(BuildSelectionRequest::Exclusion(
                EnumExclusionRequest {
                    source_span: span,
                    vocabulary,
                },
            )));
        }
    }
    let Some(
        operation @ (BuildOperation::ProviderSelection
        | BuildOperation::RepresentationSelection
        | BuildOperation::ServiceExclusion
        | BuildOperation::BoundaryAcceptance),
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
    let operand_count = if matches!(
        operation,
        BuildOperation::ServiceExclusion | BuildOperation::BoundaryAcceptance
    ) {
        1
    } else {
        2
    };
    if operands.len() != operand_count
        || evidence_count != 0
        || !(ordinary_count == 1
            || (operation == BuildOperation::ProviderSelection && ordinary_count == 2))
    {
        return Err(Diagnostic::error(format!(
            "{} requires {} and its explicit mutable Build argument",
            operation.authored_spelling(),
            if operand_count == 1 {
                "one bare declaration operand"
            } else {
                "two bare declaration operands"
            }
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
