//! Join completed Build choices to the source requests that authorized them.
//! Targets and composition come from execution; source supplies the exact slot,
//! provider, and lexical author. Nothing here executes or accepts a provider body.

use super::provider_declarations::{
    ProviderForm, ProviderPlanningContext, ProviderSelection, form, identity,
};
use diagnostics::Diagnostic;
use language_semantics::declaration_selection::{
    BuildOperation, BuildSelectionOrigin, ExecutedProductSelection, ProductDeclarationRequest,
    ProductOperandRole,
};
use std::sync::Arc;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::product_operand::{
    ProductSelectionOccurrence, collect_product_selection_occurrences,
    resolve_build_request_author, resolve_product_path,
};

struct Formation<'request> {
    request: &'request ProductDeclarationRequest,
    author: SymbolHandle,
    selection: ProviderSelection,
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

fn formation(
    context: ProviderPlanningContext<'_>,
    request: &ProductDeclarationRequest,
    author: SymbolHandle,
) -> Result<ProviderSelection, Vec<Diagnostic>> {
    request
        .validate_shape()
        .map_err(|reason| refusal(request, reason))?;
    let [slot, provider] = request.operands.as_ref() else {
        return Err(refusal(
            request,
            "provider request needs an exact slot and provider",
        ));
    };
    if request
        .operands
        .iter()
        .any(|operand| operand.application.is_some())
    {
        return Err(refusal(
            request,
            "no product route yet: provider choices with compile-time arguments",
        ));
    }
    let resolve =
        |operand: &language_semantics::declaration_selection::ProductDeclarationOperand, role| {
            resolve_product_path(context.typed, &operand.path, operand.source_span, role)
            .map(|symbol| identity(context, symbol, &operand.path))
            .ok_or_else(|| refusal(request, format!(
                "cannot select provider operand `{}` here; it must name one visible declaration of the required kind",
                operand.path,
            )))
        };
    let slot = resolve(slot, ProductOperandRole::BoundarySubject)?;
    let provider = resolve(provider, ProductOperandRole::Provider)?;
    let span = context
        .typed
        .symbols
        .rejoin_product_occurrence(request.source_span);
    let origin = BuildSelectionOrigin::capture(&context.typed.symbols, author, author, span, None)
        .map_err(|reason| refusal(request, reason))?;
    form(
        context,
        slot,
        provider,
        origin,
        None,
        Default::default(),
        ProviderForm::StaticRecord,
    )
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
            selection: formation(context, request, author)?,
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
        let author = resolve_build_request_author(
            typed,
            event.selecting_machine.ok_or_else(|| {
                refusal(request, "executed provider choice lost its lexical author")
            })?,
        )
        .map_err(|reason| refusal(request, reason))?;
        let selected = key(typed, request);
        let start = formations.partition_point(|row| key(typed, row.request) < selected);
        let formation = formations[start..].iter()
            .take_while(|row| key(typed, row.request) == selected)
            .find(|row| row.author == author && typed.same_product_request_source(row.request, request))
            .ok_or_else(|| refusal(request, "executed provider choice does not match its declared source occurrence and author"))?;
        let mut selection = formation.selection.clone();
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
