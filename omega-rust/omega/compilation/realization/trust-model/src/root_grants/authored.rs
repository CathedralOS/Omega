//! Join source-issued trust acceptances after Product declarations are available.
//! Formation checks all authored operands; only completed Build operations grant
//! acceptance. Declaration visibility does not select a provider or prove its body.

use diagnostics::Diagnostic;
use language_semantics::declaration_selection::{
    BuildOperation, ExecutedProductSelection, ProductDeclarationRequest,
};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::product_operand::{ProductOperandKind, resolve_product_path};

pub fn collect_product_root_grants(
    typed: &TypedTrees,
    activation: Option<SymbolHandle>,
    declared: &[std::sync::Arc<ProductDeclarationRequest>],
    executed: &[ExecutedProductSelection],
) -> Result<Vec<crate::AuthoredRootGrant>, Vec<Diagnostic>> {
    for request in declared
        .iter()
        .filter(|request| request.operation == BuildOperation::BoundaryAcceptance)
    {
        subject(typed, request).map_err(|error| vec![error])?;
    }
    let mut grants = Vec::new();
    for event in executed
        .iter()
        .filter(|event| event.request.operation == BuildOperation::BoundaryAcceptance)
    {
        let result = (|| {
            if !declared.contains(&event.request) || event.composition.is_some() {
                return Err(Diagnostic::error(
                    "boundary acceptance lost its source-issued declaration request",
                ));
            }
            activation
                .filter(|symbol| typed.machine(*symbol).is_some())
                .ok_or_else(|| {
                    Diagnostic::error("boundary acceptance has no authoritative Build activation")
                })?;
            let machine_span = event.selecting_machine.ok_or_else(|| {
                Diagnostic::error("boundary acceptance lost its authored machine coordinate")
            })?;
            let selecting_machine = typed
                .machines()
                .iter()
                .find(|machine| {
                    typed.symbols.symbol_source_span(machine.symbol) == Some(machine_span)
                })
                .map(|machine| machine.symbol)
                .ok_or_else(|| {
                    Diagnostic::error("boundary acceptance lost its authored selecting machine")
                })?;
            let selected = subject(typed, &event.request)?;
            Ok(crate::AuthoredRootGrant {
                selector: typed
                    .product_declarations()
                    .symbols
                    .display_path(selected, "::"),
                selecting_machine,
                source_span: event.request.source_span,
            })
        })();
        let grant = result
            .map_err(|error: Diagnostic| vec![error.with_source_span(event.request.source_span)])?;
        if !grants.contains(&grant) {
            grants.push(grant);
        }
    }
    Ok(grants)
}

fn subject(
    typed: &TypedTrees,
    request: &ProductDeclarationRequest,
) -> Result<SymbolHandle, Diagnostic> {
    let [operand] = request.operands.as_ref() else {
        return Err(
            Diagnostic::error("boundary acceptance requires one Product subject")
                .with_source_span(request.source_span),
        );
    };
    resolve_product_path(
        typed,
        &operand.path,
        operand.source_span,
        ProductOperandKind::BoundarySubject,
    )
    .ok_or_else(|| {
        Diagnostic::error(format!(
            "boundary acceptance `{}` does not resolve to one visible Product boundary subject",
            operand.path,
        ))
        .with_source_span(operand.source_span)
    })
}
