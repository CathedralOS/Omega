//! Admit source-issued Product choices after the dependency/source join.
//! Formation validates every authored request; only the authenticated execution
//! ledger activates choices and supplies their actual composition values.

use super::{ProviderPlanningContext, ProviderSelection, ProviderSelectionIdentity, admission};
use diagnostics::Diagnostic;
use language_semantics::declaration_selection::{
    BuildOperation, ExecutedProductSelection, ProductDeclarationRequest,
};
use symbols::SymbolHandle;
use typed_trees::product_operand::{ProductOperandKind, resolve_product_path};

fn identities(
    context: ProviderPlanningContext<'_>,
    request: &ProductDeclarationRequest,
) -> Result<(ProviderSelectionIdentity, ProviderSelectionIdentity), Diagnostic> {
    if context.scope != source::DependencyScope::Product
        || request.operation != BuildOperation::ProviderSelection
    {
        return Err(Diagnostic::error(
            "source-issued provider choices require their exact Product planning context",
        ));
    }
    let [subject, provider] = request.operands.as_ref() else {
        return Err(Diagnostic::error(
            "provider choice lost its two authored declaration operands",
        ));
    };
    let project =
        |operand: &language_semantics::declaration_selection::ProductDeclarationOperand, role| {
            let symbol = resolve_product_path(context.typed, &operand.path, operand.source_span, role)
            .ok_or_else(|| Diagnostic::error(format!("Product provider operand `{}` does not resolve to one visible declaration of role {role:?}", operand.path))
                .with_source_span(operand.source_span))?;
            Ok(ProviderSelectionIdentity {
                symbol,
                package: context.typed.symbols.symbol_package_identity(symbol),
                canonical_path: context.typed.symbols.display_path(symbol, "::"),
                authored_path: operand.path.clone(),
            })
        };
    Ok((
        project(subject, ProductOperandKind::BoundarySubject)?,
        project(provider, ProductOperandKind::Provider)?,
    ))
}

pub fn form_product_provider_request(
    context: ProviderPlanningContext<'_>,
    request: &ProductDeclarationRequest,
) -> Result<(), Diagnostic> {
    let (subject, provider) = identities(context, request)?;
    admission::form(context, subject, provider).map(|_| ())
}

pub fn collect_product_provider_selections(
    context: ProviderPlanningContext<'_>,
    activation: Option<SymbolHandle>,
    declared: &[std::sync::Arc<ProductDeclarationRequest>],
    executed: &[ExecutedProductSelection],
) -> Result<Vec<ProviderSelection>, Vec<Diagnostic>> {
    for request in declared
        .iter()
        .filter(|request| request.operation == BuildOperation::ProviderSelection)
    {
        form_product_provider_request(context, request)
            .map_err(|error| vec![error.with_source_span(request.source_span)])?;
    }
    let has_requests = declared
        .iter()
        .any(|request| request.operation == BuildOperation::ProviderSelection)
        || executed
            .iter()
            .any(|event| event.request.operation == BuildOperation::ProviderSelection);
    let Some(activation) = activation else {
        return if has_requests {
            Err(vec![Diagnostic::error(
                "provider requests have no authoritative Build activation",
            )])
        } else {
            Ok(Vec::new())
        };
    };
    if context.typed.machine(activation).is_none() {
        return Err(vec![Diagnostic::error(
            "provider requests lost their exact authoritative Build machine",
        )]);
    }
    let mut selections = Vec::new();
    for event in executed
        .iter()
        .filter(|event| event.request.operation == BuildOperation::ProviderSelection)
    {
        let result = (|| {
            if !declared.contains(&event.request) {
                return Err(Diagnostic::error(
                    "executed provider choice has no retained authored formation request",
                ));
            }
            let mode = event.composition.ok_or_else(|| {
                Diagnostic::error("executed provider choice lost its evaluated composition mode")
            })?;
            let (subject, provider) = identities(context, &event.request)?;
            admission::admit(
                context,
                subject,
                provider,
                mode,
                activation,
                event.request.source_span,
                &selections,
            )
        })();
        match result {
            Ok(selection) => selections.push(selection),
            Err(error) => return Err(vec![error.with_source_span(event.request.source_span)]),
        }
    }
    Ok(selections)
}
