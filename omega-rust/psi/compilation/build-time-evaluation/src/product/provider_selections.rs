//! Join completed Build choices to the source requests that authorized them.
//! Targets and composition come from execution; source supplies the exact slot,
//! provider, and lexical author. Nothing here executes or accepts a provider body.

use super::provider_declarations::{
    ProviderPlanningContext, ProviderSelection, ProviderSelectionIdentity, form, identity,
};
use diagnostics::Diagnostic;
use language_semantics::declaration_selection::product_operands::ProductStaticEnvironment;
use language_semantics::declaration_selection::{
    BuildOperation, BuildSelectionOrigin, ExecutedProductSelection, ProductDeclarationOperand,
    ProductDeclarationRequest, ProductOperandRole,
};
use std::sync::Arc;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::product_operand::{
    ProductDeclarationLookup, ProductRequestAuthors, ProductSelectionOccurrence,
    collect_product_selection_occurrences,
};

struct Formation<'request> {
    request: &'request ProductDeclarationRequest,
    author: SymbolHandle,
    /// `None` for a request authored inside a generic helper: each executed
    /// choice forms its own selection from the helper's arguments.
    selection: Option<ProviderSelection>,
}

type OccurrenceKey = (usize, usize, usize);

fn key(typed: &TypedTrees, request: &ProductDeclarationRequest) -> OccurrenceKey {
    let span = typed.symbols.rejoin_product_occurrence(request.source_span);
    (span.source_id.0, span.span.start, span.span.end)
}

fn refusal(request: &ProductDeclarationRequest, reason: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::error(reason).with_source_span(request.source_span)]
}

fn author(
    typed: &TypedTrees,
    occurrences: &[ProductSelectionOccurrence],
    request: &ProductDeclarationRequest,
) -> Result<SymbolHandle, Vec<Diagnostic>> {
    let selected = key(typed, request);
    let start = occurrences.partition_point(|row| key(typed, &row.request) < selected);
    let mut owners = occurrences[start..]
        .iter()
        .take_while(|row| key(typed, &row.request) == selected)
        .filter(|row| typed.same_product_request_source(&row.request, request))
        .map(|row| typed.symbols.authored_origin(row.machine));
    let owner = owners.next().ok_or_else(|| {
        refusal(
            request,
            "provider request has no original authored occurrence",
        )
    })?;
    if owners.any(|other| other != owner) {
        return Err(refusal(
            request,
            "provider request has ambiguous lexical authors",
        ));
    }
    if !typed
        .symbols
        .symbol_is_in_dependency_scope(owner, source::DependencyScope::Build)
    {
        return Err(refusal(
            request,
            "provider request author is not Build source",
        ));
    }
    Ok(owner)
}

/// One operand of a request as the declaration it names and, when it is
/// applied to static arguments, the application typed preparation formed
/// for it (`typed_trees_to_typed_trees::prepare_product_operand_application`).
struct Operand {
    identity: ProviderSelectionIdentity,
    /// The operand names a parameter of the generic helper that wrote it,
    /// and this reading has no environment: the request as authored.
    open: bool,
}

fn operand(
    context: ProviderPlanningContext<'_>,
    declarations: &ProductDeclarationLookup<'_>,
    request: &ProductDeclarationRequest,
    operand: &ProductDeclarationOperand,
    role: ProductOperandRole,
    author: SymbolHandle,
    environment: Option<&ProductStaticEnvironment>,
) -> Result<Operand, Vec<Diagnostic>> {
    let typed = context.typed;
    let symbol = declarations.resolve(&operand.path, operand.source_span, role)
        .ok_or_else(|| {
            refusal(
                request,
                format!(
                    "cannot select provider operand `{}` here; it must name one visible declaration of the required kind",
                    operand.path,
                ),
            )
        })?;
    let mut identity = identity(context, symbol, &operand.path);
    if operand.application.is_none() {
        return Ok(Operand {
            identity,
            open: false,
        });
    }
    let Some(application) = typed.prepared_product_operand(operand, environment, author, role)
    else {
        // Only the authored request of a generic helper is read without
        // its application: an executed choice always has one.
        return if environment.is_none() {
            Ok(Operand {
                identity,
                open: true,
            })
        } else {
            Err(refusal(
                request,
                "executed provider choice lost the application of its operand",
            ))
        };
    };
    let text = typed
        .product_operand_application_identity(application)
        .map_err(|reason| refusal(request, reason))?
        .map(|identity| {
            typed
                .type_identity_text(identity)
                .ok_or_else(|| refusal(request, "provider application lost its canonical identity"))
        })
        .transpose()?;
    identity.application = application;
    identity.applied = text.is_some();
    identity.application_identity = text;
    Ok(Operand {
        identity,
        open: false,
    })
}

/// The application of its trait a provider satisfies: the one typed
/// preparation retained for an applied provider, the declared one for a
/// provider with no parameters of its own.
fn satisfied_application(
    typed: &TypedTrees,
    provider: &ProviderSelectionIdentity,
) -> Option<(SymbolHandle, String)> {
    let scope = if provider.applied {
        typed
            .product_operand_applications
            .get(provider.application)
            .scope
            .scope(typed)?
    } else {
        typed_trees::trait_definition::AppliedTraitScope::from_conformance(
            typed,
            typed.conformance_by_symbol(provider.symbol)?,
        )?
    };
    let identity = scope.portable_application_identity(typed)?;
    Some((
        scope.declaration.symbol,
        typed.type_identity_text(identity)?,
    ))
}

/// The selection a request describes under `environment`, or `None` for a
/// request authored inside a generic helper and read without the helper's
/// arguments: its operands name no application yet.
fn formation(
    context: ProviderPlanningContext<'_>,
    declarations: &ProductDeclarationLookup<'_>,
    request: &ProductDeclarationRequest,
    author: SymbolHandle,
    environment: Option<&ProductStaticEnvironment>,
) -> Result<Option<ProviderSelection>, Vec<Diagnostic>> {
    request
        .validate_shape()
        .map_err(|reason| refusal(request, reason))?;
    let [slot, provider] = request.operands.as_ref() else {
        return Err(refusal(
            request,
            "provider request needs an exact slot and provider",
        ));
    };
    let slot = operand(
        context,
        declarations,
        request,
        slot,
        ProductOperandRole::BoundarySubject,
        author,
        environment,
    )?;
    let provider = operand(
        context,
        declarations,
        request,
        provider,
        ProductOperandRole::Provider,
        author,
        environment,
    )?;
    if slot.open || provider.open {
        return Ok(None);
    }
    // provider_selection.md, "Generic selected requirements": "Keep
    // concrete generic arguments in the closure keys rather than collapsing
    // `Stage<1>` and `Stage<2>` to one trait or provider declaration." A
    // provider chosen for `Stage<1>` must therefore satisfy `Stage<1>`
    // itself, not only the trait `Stage`.
    if let Some(selected) = &slot.identity.application_identity {
        let satisfied = satisfied_application(context.typed, &provider.identity);
        if satisfied.as_ref().is_none_or(|(declaration, application)| {
            *declaration != slot.identity.symbol || application != selected
        }) {
            return Err(refusal(
                request,
                format!(
                    "provider `{}` does not satisfy the selected application of `{}`: it satisfies `{}` and the choice selects `{selected}`",
                    provider.identity.authored_path,
                    slot.identity.authored_path,
                    satisfied
                        .map_or_else(|| "no complete application".to_owned(), |(_, text)| text),
                ),
            ));
        }
    }
    let span = context
        .typed
        .symbols
        .rejoin_product_occurrence(request.source_span);
    let origin = BuildSelectionOrigin::capture(&context.typed.symbols, author, author, span, None)
        .map_err(|reason| refusal(request, reason))?;
    form(
        context,
        slot.identity,
        provider.identity,
        origin,
        None,
        Default::default(),
    )
    .map(Some)
    .map_err(|diagnostic| vec![diagnostic.with_source_span(request.source_span)])
}

pub(super) fn collect(
    context: ProviderPlanningContext<'_>,
    activation: Option<SymbolHandle>,
    declared: &[Arc<ProductDeclarationRequest>],
    executed: &[ExecutedProductSelection],
) -> Result<Vec<ProviderSelection>, Vec<Diagnostic>> {
    let mut requests = declared
        .iter()
        .filter(|request| request.operation == BuildOperation::ProviderSelection)
        .collect::<Vec<_>>();
    let mut events = executed
        .iter()
        .enumerate()
        .filter(|(_, event)| event.request.operation == BuildOperation::ProviderSelection)
        .peekable();
    if requests.is_empty() && events.peek().is_none() {
        return Ok(Vec::new());
    }
    let typed = context.typed;
    let declarations = ProductDeclarationLookup::new(
        typed,
        &[
            ProductOperandRole::BoundarySubject,
            ProductOperandRole::Provider,
        ],
    );
    let authors = ProductRequestAuthors::new(typed);
    let mut occurrences = collect_product_selection_occurrences(typed);
    occurrences.retain(|row| row.request.operation == BuildOperation::ProviderSelection);
    occurrences.sort_unstable_by_key(|row| key(typed, &row.request));
    requests.sort_unstable_by_key(|request| key(typed, request));
    let mut formations = Vec::with_capacity(requests.len());
    // Validate unexecuted requests too: lack of an event cannot hide an invalid
    // declaration selection. Keep source lookup separate from executed order.
    for request in requests {
        let author = author(typed, &occurrences, request)?;
        formations.push(Formation {
            request,
            author,
            selection: formation(context, &declarations, request, author, None)?,
        });
    }
    let mut selections: Vec<ProviderSelection> = Vec::new();
    for (ordinal, event) in events {
        let request = &event.request;
        let activation = activation
            .filter(|symbol| {
                typed.machine_by_symbol(*symbol).is_some()
                    && typed
                        .symbols
                        .symbol_is_in_dependency_scope(*symbol, source::DependencyScope::Build)
            })
            .ok_or_else(|| {
                refusal(
                    request,
                    "executed provider choice has no original Build activation",
                )
            })?;
        let author = authors
            .resolve(event.selecting_machine.ok_or_else(|| {
                refusal(request, "executed provider choice lost its lexical author")
            })?)
            .map_err(|reason| refusal(request, reason))?;
        let selected = key(typed, request);
        let start = formations.partition_point(|row| key(typed, row.request) < selected);
        let formation = formations[start..].iter()
            .take_while(|row| key(typed, row.request) == selected)
            .find(|row| row.author == author && typed.same_product_request_source(row.request, request))
            .ok_or_else(|| refusal(request, "executed provider choice does not match its declared source occurrence and author"))?;
        // A choice whose operands are applied is formed from what this
        // execution gave the helper that made it: two calls of one helper
        // share the authored request and differ in their arguments.
        let applied = request
            .operands
            .iter()
            .any(|operand| operand.application.is_some());
        let formed = if applied {
            self::formation(
                context,
                &declarations,
                request,
                author,
                event.static_environment.as_deref(),
            )?
        } else {
            formation.selection.clone()
        };
        let mut selection = formed.ok_or_else(|| {
            refusal(
                request,
                "executed provider choice names a helper parameter with no value",
            )
        })?;
        selection.target = Some(event.target.ok_or_else(|| {
            refusal(
                request,
                "executed provider choice lost its evaluated target",
            )
        })?);
        selection.composition_mode = event.composition.ok_or_else(|| {
            refusal(
                request,
                "executed provider choice lost its evaluated composition mode",
            )
        })?;
        let span = typed.symbols.rejoin_product_occurrence(request.source_span);
        selection.origin = BuildSelectionOrigin::capture(
            &typed.symbols,
            activation,
            author,
            span,
            Some(u64::try_from(ordinal).map_err(|_| {
                refusal(request, "provider event ordinal exceeds its representation")
            })?),
        )
        .map_err(|reason| refusal(request, reason))?;
        // Preserve every choice and its order. Provider planning owns overlap,
        // composition, and complete coverage against the selected schemas.
        selections.push(selection);
    }
    Ok(selections)
}
