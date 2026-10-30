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
    static_environment: Option<
        &language_semantics::declaration_selection::product_operands::ProductStaticEnvironment,
    >,
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
        |operand: &language_semantics::declaration_selection::ProductDeclarationOperand,
         role|
         -> Result<ProviderSelectionIdentity, Diagnostic> {
            let symbol = resolve_product_path(context.typed, &operand.path, operand.source_span, role)
            .ok_or_else(|| Diagnostic::error(format!("Product provider operand `{}` does not resolve to one visible declaration of role {role:?}", operand.path))
                .with_source_span(operand.source_span))?;
            let application = if operand.application.is_some() {
                let mut matching = context
                    .typed
                    .live_product_operand_applications()
                    .into_iter()
                    .filter(|handle| {
                        let prepared = context.typed.product_operand_applications.get(*handle);
                        prepared.role == Some(role)
                            && prepared.environment.as_deref() == static_environment
                            && context
                                .typed
                                .same_product_operand_source(&prepared.operand, operand)
                    });
                let selected = matching.next().ok_or_else(|| {
                    Diagnostic::error(
                        "Product provider operand lost its original prepared application",
                    )
                    .with_source_span(operand.source_span)
                })?;
                if matching.next().is_some() {
                    return Err(Diagnostic::error(
                        "Product provider operand has multiple prepared lexical applications",
                    )
                    .with_source_span(operand.source_span));
                }
                selected
            } else {
                let incomplete = context
                    .typed
                    .trait_definition_by_symbol(symbol)
                    .is_some_and(|definition| {
                        !context.typed.trait_type_parameters(definition).is_empty()
                    })
                    || context
                        .typed
                        .conformance_by_symbol(symbol)
                        .is_some_and(|definition| {
                            !context
                                .typed
                                .conformance_type_parameters(definition)
                                .is_empty()
                        });
                if incomplete {
                    return Err(Diagnostic::error(
                        "provider choice must supply the declaration's complete static application",
                    )
                    .with_source_span(operand.source_span));
                }
                Default::default()
            };
            let application_identity = if application.is_valid() {
                context
                    .typed
                    .product_operand_application_identity(application)
                    .map_err(|reason| {
                        Diagnostic::error(reason).with_source_span(operand.source_span)
                    })?
                    .map(|identity| {
                        context.typed.type_identity_text(identity).ok_or_else(|| {
                            Diagnostic::error("Product application lost its canonical identity")
                        })
                    })
                    .transpose()?
            } else {
                None
            };
            Ok(ProviderSelectionIdentity {
                application,
                application_identity,
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

/// Build checks inert declaration roles against its complete Product
/// descriptions. Actual recipes remain obligations of the joined Product;
/// a successful discovery/description grants no closed application or provider.
pub fn validate_product_provider_description(
    context: ProviderPlanningContext<'_>,
    request: &ProductDeclarationRequest,
) -> Result<(), Diagnostic> {
    let context = ProviderPlanningContext::new(context.typed.product_declarations(), context.scope);
    if context.scope != source::DependencyScope::Product
        || request.operation != BuildOperation::ProviderSelection
    {
        return Err(Diagnostic::error(
            "provider descriptions require the exact Product declaration frontier",
        ));
    }
    let [subject, provider] = request.operands.as_ref() else {
        return Err(Diagnostic::error(
            "provider description lost its exact two operands",
        ));
    };
    let describe =
        |operand: &language_semantics::declaration_selection::ProductDeclarationOperand,
         role|
         -> Result<ProviderSelectionIdentity, Diagnostic> {
            let symbol =
                resolve_product_path(context.typed, &operand.path, operand.source_span, role)
                    .ok_or_else(|| {
                        Diagnostic::error(
                            "provider description does not name one exact visible declaration",
                        )
                        .with_source_span(operand.source_span)
                    })?;
            Ok(ProviderSelectionIdentity {
                symbol,
                application: Default::default(),
                application_identity: None,
                package: context.typed.symbols.symbol_package_identity(symbol),
                canonical_path: context.typed.symbols.display_path(symbol, "::"),
                authored_path: operand.path.clone(),
            })
        };
    admission::form(
        context,
        describe(subject, ProductOperandKind::BoundarySubject)?,
        describe(provider, ProductOperandKind::Provider)?,
    )
    .map(|_| ())
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
        let (subject, provider) = identities(context, request, None)
            .map_err(|error| vec![error.with_source_span(request.source_span)])?;
        admission::form(context, subject, provider)
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
    for (ordinal, event) in executed
        .iter()
        .enumerate()
        .filter(|(_, event)| event.request.operation == BuildOperation::ProviderSelection)
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
            let target = event.target.ok_or_else(|| {
                Diagnostic::error("executed provider choice lost its evaluated catalog target")
            })?;
            let (subject, provider) =
                identities(context, &event.request, event.static_environment.as_deref())?;
            let author = typed_trees::product_operand::resolve_build_request_author(
                context.typed,
                event.selecting_machine.ok_or_else(|| {
                    Diagnostic::error("executed provider choice lost its lexical author")
                })?,
            )
            .map_err(Diagnostic::error)?;
            let origin = language_semantics::declaration_selection::BuildSelectionOrigin::capture(
                &context.typed.symbols,
                activation,
                author,
                context
                    .typed
                    .symbols
                    .rejoin_product_occurrence(event.request.source_span),
                Some(ordinal as u64),
            )
            .map_err(Diagnostic::error)?;
            admission::admit(
                context,
                subject,
                provider,
                mode,
                Some(target),
                origin,
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
