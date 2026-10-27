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
    typed: &mut typed_trees::TypedTrees,
    exposure: Option<
        language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure,
    >,
    namespace: symbols::SymbolHandle,
    target: symbols::SymbolHandle,
    name: &str,
    operands: &[symbol_resolved_trees::expression::StaticMachineArgument],
    ordinary_count: usize,
    evidence_count: usize,
    span: source::SourceSpan,
) -> Result<Option<typed_trees::expression::BuildSelectionRequest>, Diagnostic> {
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
    let Some(operation) = BuildOperation::from_call_target(name)
        .filter(|operation| operation.has_designated_operands())
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
    let operand_count_valid = if operation == BuildOperation::ProviderReadiness {
        operands.len() == 2
    } else {
        operation.product_operand_count_valid(operands.len())
    };
    if !operand_count_valid
        || evidence_count != 0
        || !(ordinary_count == 1
            || (operation == BuildOperation::ProviderSelection && ordinary_count == 2))
    {
        return Err(Diagnostic::error(format!(
            "{} requires {} and its explicit mutable Build argument",
            operation.authored_spelling(),
            match operation {
                BuildOperation::ServiceExclusion
                | BuildOperation::ServiceAudit
                | BuildOperation::BoundaryAcceptance => "one bare declaration operand",
                BuildOperation::WireCompatibilityRequest =>
                    "four schema declaration operands and compatibility facts",
                _ => "two bare declaration operands",
            },
        ))
        .with_source_span(span));
    }
    let requirement = if operation == BuildOperation::ProviderReadiness {
        let operand = super::expression::lower_static_machine_argument(
            Some(source),
            typed,
            exposure,
            &operands[0],
        )?;
        let reference = typed
            .static_operand_type_reference(&operand)
            .map_err(|reason| Diagnostic::error(reason).with_source_span(span))?;
        let requirement = typed.type_reference_table.type_symbol(reference);
        if !source
            .traits
            .iter()
            .any(|definition| definition.symbol == requirement && definition.is_boundary)
        {
            return Err(Diagnostic::error("provider readiness requires a pre-known exact boundary requirement; only the backing may be deferred")
                .with_source_span(span));
        }
        Some(reference)
    } else {
        None
    };
    let operands = operands.iter().skip(usize::from(requirement.is_some())).map(|operand| {
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
    if let Some(requirement) = requirement {
        return Ok(Some(BuildSelectionRequest::ProviderReadiness(
            language_semantics::declaration_selection::ProviderReadinessRequest {
                source_span: span,
                requirement,
                backing: operands[0].clone(),
            },
        )));
    }
    Ok(Some(BuildSelectionRequest::Declaration(
        std::sync::Arc::new(ProductDeclarationRequest {
            source_span: span,
            operation,
            operands,
        }),
    )))
}
