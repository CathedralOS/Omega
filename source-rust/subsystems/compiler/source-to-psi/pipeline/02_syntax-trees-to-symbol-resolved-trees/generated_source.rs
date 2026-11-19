use crate::resolution::{
    DeferredResolvedFrontier, ResolutionRequest, begin_complete,
    resolve_deferred_with_declarations, resolve_forest,
};
use crate::source_inputs::SourceMap;
use crate::source_resolved::SymbolResolvedTrees;
use crate::source_resolved::continuations::{
    RebasedSeededSymbolResolvedTrees, SeededSymbolResolvedTrees,
};
use diagnostics::Diagnostic;
use std::sync::Arc;
use syntax_trees::SyntaxTrees;

/* A later-stratum forest appended to an exact retained base. */
pub struct ExtensionRequest {
    /*
        Shared with the typed base that owns it; the extension reads it and
        never needs a private copy of the whole predecessor.
    */
    pub base: Arc<SymbolResolvedTrees>,
    /*
        The authored forest the base was resolved from. A transparent alias
        expands from its authored right-hand side before any symbol exists, so
        an extension applying an alias the base declares (std applying core's
        `CLayout<T>`) needs that declaration's syntax; the resolved base keeps
        only the alias's resolved expansion. Shared, never copied.
    */
    pub base_declarations: Arc<SyntaxTrees>,
    pub syntax: SyntaxTrees,
    /* Must retain the base's exact source frontier. */
    pub sources: Arc<SourceMap>,
    pub top_level_bindings: Vec<crate::source_symbols::SourceScopedTopLevelBinding>,
}

/*
    Append authored declarations to an already activated source base. Their
    implementations remain pending, exactly as in a fresh source request.
*/
pub fn resolve_deferred_extension(
    request: ExtensionRequest,
) -> Result<DeferredResolvedExtension, Vec<Diagnostic>> {
    let ExtensionRequest {
        base,
        base_declarations,
        syntax,
        sources,
        top_level_bindings,
    } = request;
    let predecessor = base;
    let successor = resolve_deferred_with_declarations(
        ResolutionRequest {
            syntax,
            sources: Some(sources),
            top_level_bindings,
            retained_base: Some(Arc::clone(&predecessor)),
        },
        Some(&base_declarations),
    )?;
    Ok(DeferredResolvedExtension {
        successor,
        predecessor,
    })
}

/* New deferred syntax and its exact resolved predecessor travel together. */
pub struct DeferredResolvedExtension {
    successor: DeferredResolvedFrontier,
    predecessor: Arc<SymbolResolvedTrees>,
}

impl DeferredResolvedExtension {
    /*
        Join the typed predecessor's selection suffix once, on both the
        declaration input and the frontier which will later complete its bodies.
    */
    pub fn rebase_for_typed_continuation(
        mut self,
        selections: &crate::source_resolved::AuthoredDeclarationSelections,
    ) -> Result<(RebasedSeededSymbolResolvedTrees, DeferredResolvedFrontier), Vec<Diagnostic>> {
        let source = SeededSymbolResolvedTrees {
            trees: Arc::unwrap_or_clone(std::mem::take(&mut self.successor.trees)),
            authored_selection_frontier: self.predecessor.authored_selection_extension_frontier(),
            retained_base: self.predecessor,
        }
        .rebase_authored_selections_for_typed_continuation(selections)
        .map_err(|(_, reason)| {
            vec![Diagnostic::error(format!(
                "authored declaration extension lost its typed selection prefix: {reason:?}"
            ))]
        })?;
        self.successor.trees = Arc::new(source.trees().clone());
        Ok((source, self.successor))
    }
}

/*
    Resolve one syntax extension against its retained base while keeping the
    exact append frontier of every authored-selection occurrence store.

    Existing arenas and symbol tables are consumed and extended in place; no
    source bytes are read and neither forest is parsed again. The returned
    carrier is readable, but its trees can enter a later seeded phase only by
    transactionally rebasing the extension suffix against that phase's exact
    retained authored-selection ledger.
*/
pub fn resolve_extension(
    request: ExtensionRequest,
) -> Result<SeededSymbolResolvedTrees, Vec<Diagnostic>> {
    let ExtensionRequest {
        base,
        base_declarations,
        syntax,
        sources,
        top_level_bindings,
    } = request;
    let authored_selection_frontier = base.authored_selection_extension_frontier();
    let retained_base = Arc::clone(&base);
    let request = ResolutionRequest {
        syntax,
        sources: Some(sources),
        top_level_bindings,
        retained_base: Some(base),
    };
    let (syntax, lowerer) = begin_complete(request, Some(&base_declarations))?;
    let (trees, _, _) = resolve_forest(lowerer, &syntax)?;
    Ok(SeededSymbolResolvedTrees {
        trees,
        authored_selection_frontier,
        retained_base,
    })
}
