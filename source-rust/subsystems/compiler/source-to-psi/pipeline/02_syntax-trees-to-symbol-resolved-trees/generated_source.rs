use crate::resolution::{DeferredResolvedFrontier, ResolutionRequest, resolve_deferred};
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
        syntax,
        sources,
        top_level_bindings,
    } = request;
    let predecessor = base;
    let successor = resolve_deferred(ResolutionRequest {
        syntax,
        sources: Some(sources),
        top_level_bindings,
        retained_base: Some(Arc::clone(&predecessor)),
    })?;
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

/// Materialize the fixture's complete forest through ordinary body completion.
#[cfg(any(test, feature = "test-support"))]
pub fn resolve_extension(
    request: ExtensionRequest,
) -> Result<SeededSymbolResolvedTrees, Vec<Diagnostic>> {
    let extension = resolve_deferred_extension(request)?;
    Ok(SeededSymbolResolvedTrees {
        trees: extension.successor.into_complete_trees()?,
        authored_selection_frontier: extension
            .predecessor
            .authored_selection_extension_frontier(),
        retained_base: extension.predecessor,
    })
}
