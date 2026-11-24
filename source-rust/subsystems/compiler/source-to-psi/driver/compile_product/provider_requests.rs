use typed_trees::TypedTrees;
use crate::declaration_selection::BuildOperation;
use crate::declaration_selection::ExecutedProductSelection;
use crate::declaration_selection::ProductDeclarationRequest;
use crate::declaration_selection::product_operands::ProductStaticEnvironment;
use crate::compile_product::provider_declarations::{
    ProviderPlanningContext, ProviderSelection, collect_product_provider_selections,
};
use crate::source_state::product_operand::ProductApplicationDemand;
use diagnostics::Diagnostic;
use std::sync::Arc;
use symbols::SymbolHandle;

pub(super) fn admit_product_provider_requests(
    typed: &mut TypedTrees,
    activation: Option<SymbolHandle>,
    declared: &[Arc<ProductDeclarationRequest>],
    executed: &[ExecutedProductSelection],
    formations: &[(Arc<ProductDeclarationRequest>, SymbolHandle)],
) -> Result<Vec<ProviderSelection>, Vec<Diagnostic>> {
    prepare_product_applications(typed, formations, executed, |operation| {
        operation != BuildOperation::TestGroupRegistration
    })?;
    collect_product_provider_selections(
        ProviderPlanningContext::new(
            typed,
            source_provenance::DependencyScope::Product,
        ),
        activation,
        declared,
        executed,
    )
}

/// Form each operand in its authored scope and apply the exact executed environment.
pub(super) fn prepare_product_applications(
    typed: &mut TypedTrees,
    formations: &[(Arc<ProductDeclarationRequest>, SymbolHandle)],
    executed: &[ExecutedProductSelection],
    select: impl Fn(BuildOperation) -> bool,
) -> Result<(), Vec<Diagnostic>> {
    let applies = |request: &ProductDeclarationRequest| {
        select(request.operation) && has_product_application(request)
    };
    if formations
        .iter()
        .any(|(request, _)| applies(request))
        || executed
            .iter()
            .any(|event| applies(&event.request))
    {
        // Applying an operand mutates the consuming program, not the source
        // declarations and lexical authors that authorized this batch.
        let source = typed.clone();
        let lookups = crate::source_state::product_operand::ProductSourceLookups::new(
            &source,
        );
        for (request, author) in formations
            .iter()
            .filter(|(request, _)| applies(request))
        {
            prepare_application_request(
                typed,
                &lookups,
                request,
                *author,
                None,
                ProductApplicationDemand::FormationOnly,
            )?;
        }
        for event in executed
            .iter()
            .filter(|event| applies(&event.request))
        {
            let author = lookups
                .authors
                .resolve(event.selecting_machine.ok_or_else(|| {
                    vec![Diagnostic::error(
                        "executed provider recipe lost its original author",
                    )]
                })?)
                .map_err(|reason| vec![Diagnostic::error(reason)])?;
            prepare_application_request(
                typed,
                &lookups,
                &event.request,
                author,
                event.static_environment.as_deref(),
                ProductApplicationDemand::Selected,
            )?;
        }
    }
    Ok(())
}

pub(super) fn product_formation_authors(
    typed: &TypedTrees,
    declared: &[Arc<ProductDeclarationRequest>],
    selects: impl Fn(&ProductDeclarationRequest) -> bool,
) -> Result<Vec<(Arc<ProductDeclarationRequest>, SymbolHandle)>, Vec<Diagnostic>> {
    let declared = declared
        .iter()
        .filter(|request| selects(request))
        .collect::<Vec<_>>();
    if declared.is_empty() {
        return Ok(Vec::new());
    }
    // One source roster serves this policy batch. The span narrows the
    // join; the complete request still proves lexical author custody.
    let occurrences =
        crate::source_state::product_operand::collect_product_selection_occurrences(typed);
    formation_authors(typed, &declared, &occurrences)
}

fn prepare_application_request(
    typed: &mut TypedTrees,
    source: &crate::source_state::product_operand::ProductSourceLookups<'_>,
    request: &ProductDeclarationRequest,
    author: SymbolHandle,
    static_environment: Option<&ProductStaticEnvironment>,
    demand: ProductApplicationDemand,
) -> Result<(), Vec<Diagnostic>> {
    for (operand_index, operand) in request.operands.iter().enumerate() {
        let role = request
            .operation
            .product_operand_role(operand_index)
            .ok_or_else(|| {
                vec![Diagnostic::error("Product application lost its operand role")]
            })?;
        typed_trees::product_operand::formation::prepare_product_operand_application(
            typed,
            source,
            operand,
            role,
            author,
            static_environment,
            demand,
        )?;
    }
    Ok(())
}

fn formation_authors(
    typed: &crate::source_state::TypedTrees,
    declared: &[&Arc<ProductDeclarationRequest>],
    occurrences: &[crate::source_state::product_operand::ProductSelectionOccurrence],
) -> Result<Vec<(Arc<ProductDeclarationRequest>, SymbolHandle)>, Vec<Diagnostic>> {
    let source_key = |span: source_text::SourceSpan| {
        let span = typed.symbols.rejoin_product_occurrence(span);
        (span.source_id.0, span.span.start, span.span.end)
    };
    let mut occurrences = occurrences
        .iter()
        .map(|occurrence| (source_key(occurrence.request.source_span), occurrence))
        .collect::<Vec<_>>();
    occurrences.sort_by_key(|(span, _)| *span);
    let mut formations = Vec::new();
    for &request in declared {
        let key = source_key(request.source_span);
        let first = occurrences.partition_point(|(span, _)| *span < key);
        let last = occurrences.partition_point(|(span, _)| *span <= key);
        let mut owners = occurrences[first..last]
            .iter()
            .filter(|(_, occurrence)| {
                typed.same_product_request_source(&occurrence.request, request)
            })
            .map(|(_, occurrence)| typed.symbols.authored_origin(occurrence.machine));
        let owner = owners.next().ok_or_else(|| {
            vec![
                Diagnostic::error(
                    "Product declaration recipe lost its original authored body occurrence",
                )
                .with_source_span(request.source_span),
            ]
        })?;
        if owners.any(|other| other != owner) {
            return Err(vec![
                Diagnostic::error(
                    "Product declaration recipe has multiple original lexical owners",
                )
                .with_source_span(request.source_span),
            ]);
        }
        formations.push((Arc::clone(request), owner));
    }
    Ok(formations)
}

pub(super) fn has_product_application(request: &ProductDeclarationRequest) -> bool {
    matches!(
        request.operation,
        BuildOperation::ProviderSelection
            | BuildOperation::BoundaryAcceptance
            | BuildOperation::TestGroupRegistration
    ) && request
        .operands
        .iter()
        .any(|operand| operand.application.is_some())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::declaration_selection::ProductDeclarationOperand;
    use crate::source_state::product_operand::ProductSelectionOccurrence;

    #[test]
    fn formation_authors_keep_complete_requests_and_lexical_owners() {
        let mut builder = crate::source_symbols::SymbolTableBuilder::new();
        let root = builder.insert_root(
            symbols::SymbolKind::Root,
            symbols::SymbolNameRef::Static("root"),
        );
        let owners = builder.insert_children(
            root,
            ["first", "second"].map(|name| {
                (
                    symbols::SymbolKind::Machine,
                    symbols::SymbolNameRef::Static(name),
                )
            }),
        );
        let typed = crate::source_state::TypedTrees {
            symbols: builder.finish(),
            ..Default::default()
        };
        let span = source_text::SourceSpan::new(
            source_text::SourceId(0),
            source_text::Span::new(4, 9),
        );
        let request = Arc::new(ProductDeclarationRequest {
            operation: BuildOperation::ServiceAudit,
            source_span: span,
            operands: vec![ProductDeclarationOperand {
                path: "Boundary".into(),
                source_span: span,
                ..Default::default()
            }]
            .into(),
        });
        let occurrence = |request, machine| ProductSelectionOccurrence {
            request,
            machine,
            state: SymbolHandle::invalid(),
            expression: crate::source_state::expression::ExpressionHandle::invalid(),
        };
        let mut other = request.as_ref().clone();
        other.operands[0].path = "Other".into();
        let mut occurrences = vec![
            occurrence(Arc::new(other), owners.handle_at(1)),
            occurrence(Arc::clone(&request), owners.start()),
            occurrence(Arc::clone(&request), owners.start()),
        ];
        let authors = formation_authors(&typed, &[&request], &occurrences).unwrap();
        assert_eq!(authors.len(), 1);
        assert_eq!(authors[0].1, owners.start());
        occurrences.push(occurrence(Arc::clone(&request), owners.handle_at(1)));
        assert!(
            formation_authors(&typed, &[&request], &occurrences).unwrap_err()[0]
                .message
                .contains("multiple original lexical owners")
        );
        assert!(
            formation_authors(&typed, &[&request], &[]).unwrap_err()[0]
                .message
                .contains("lost its original authored body occurrence")
        );
    }
}
