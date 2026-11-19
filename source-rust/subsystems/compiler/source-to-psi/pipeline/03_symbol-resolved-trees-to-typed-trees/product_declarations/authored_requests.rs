//! Match completed Build requests to their original occurrence and lexical author.

use crate::declaration_selection::{
    BuildOperation, ExecutedProductSelection, ProductDeclarationRequest,
};
use crate::source_state::TypedTrees;
use crate::source_state::product_operand::{
    ProductRequestAuthors, ProductSelectionOccurrence, collect_product_selection_occurrences,
};
use diagnostics::Diagnostic;
use std::sync::Arc;
use symbols::SymbolHandle;

pub(super) fn has_operation(
    declared: &[Arc<ProductDeclarationRequest>],
    executed: &[ExecutedProductSelection],
    operation: BuildOperation,
) -> bool {
    declared.iter().any(|request| request.operation == operation)
        || executed.iter().any(|event| event.request.operation == operation)
}

type OccurrenceKey = (usize, usize, usize);

fn key(typed: &TypedTrees, request: &ProductDeclarationRequest) -> OccurrenceKey {
    let span = typed.symbols.rejoin_product_occurrence(request.source_span);
    (span.source_id.0, span.span.start, span.span.end)
}

pub fn refusal(
    request: &ProductDeclarationRequest,
    reason: impl Into<String>,
) -> Vec<Diagnostic> {
    vec![Diagnostic::error(reason).with_source_span(request.source_span)]
}

fn authored_owner(
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
    let owner = owners
        .next()
        .ok_or_else(|| refusal(request, "Build request has no original authored occurrence"))?;
    if owners.any(|other| other != owner) {
        return Err(refusal(
            request,
            "Build request has ambiguous lexical authors",
        ));
    }
    if !typed
        .symbols
        .symbol_is_in_dependency_scope(owner, source_provenance::DependencyScope::Build)
    {
        return Err(refusal(request, "Build request author is not Build source"));
    }
    Ok(owner)
}

pub struct DeclaredRequest<'request> {
    pub request: &'request ProductDeclarationRequest,
    pub author: SymbolHandle,
}

pub struct AuthoredRequests<'program, 'request> {
    typed: &'program TypedTrees,
    authors: ProductRequestAuthors<'program>,
    pub(super) rows: Vec<DeclaredRequest<'request>>,
}

impl<'program, 'request> AuthoredRequests<'program, 'request> {
    pub fn rows(&self) -> &[DeclaredRequest<'request>] {
        &self.rows
    }

    pub fn new(
        typed: &'program TypedTrees,
        declared: &'request [Arc<ProductDeclarationRequest>],
        operation: BuildOperation,
    ) -> Result<Self, Vec<Diagnostic>> {
        let mut occurrences = collect_product_selection_occurrences(typed);
        occurrences.retain(|row| row.request.operation == operation);
        occurrences.sort_unstable_by_key(|row| key(typed, &row.request));
        let mut requests = declared
            .iter()
            .filter(|request| request.operation == operation)
            .collect::<Vec<_>>();
        requests.sort_unstable_by_key(|request| key(typed, request));
        let rows = requests
            .into_iter()
            .map(|request| {
                let author = authored_owner(typed, &occurrences, request)?;
                Ok(DeclaredRequest { request, author })
            })
            .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
        Ok(Self {
            typed,
            authors: ProductRequestAuthors::new(typed),
            rows,
        })
    }

    pub fn execution(
        &self,
        activation: Option<SymbolHandle>,
        event: &ExecutedProductSelection,
    ) -> Result<(usize, SymbolHandle, SymbolHandle), Vec<Diagnostic>> {
        let request = &event.request;
        let typed = self.typed;
        let activation = activation
            .filter(|symbol| {
                typed.machine_by_symbol(*symbol).is_some()
                    && typed.symbols.symbol_is_in_dependency_scope(
                        *symbol,
                        source_provenance::DependencyScope::Build,
                    )
            })
            .ok_or_else(|| {
                refusal(
                    request,
                    "executed Build request has no original Build activation",
                )
            })?;
        let author = self
            .authors
            .resolve(event.selecting_machine.ok_or_else(|| {
                refusal(request, "executed Build request lost its lexical author")
            })?)
            .map_err(|reason| refusal(request, reason))?;
        let selected = key(typed, request);
        let start = self
            .rows
            .partition_point(|row| key(typed, row.request) < selected);
        let index = self.rows[start..].iter().take_while(|row| key(typed, row.request) == selected)
            .position(|row| row.author == author && typed.same_product_request_source(row.request, request))
            .map(|index| start + index)
            .ok_or_else(|| refusal(request, "executed Build request does not match its declared source occurrence and author"))?;
        Ok((index, author, activation))
    }
}
