//! Resolve and normalize temporary syntax with the originating loader's custody.

use std::sync::Arc;

pub(crate) fn normalize_generic_data(
    syntax_trees: syntax_trees::SyntaxTrees,
    sources: Option<Arc<source::SourceMap>>,
    source_scoped_top_level_bindings: &[symbols::SourceScopedTopLevelBinding],
    retained_base: Option<&symbol_resolved_trees::SymbolResolvedTrees>,
) -> Result<syntax_trees::SyntaxTrees, Vec<diagnostics::Diagnostic>> {
    // Scoped bindings and a retained base travel with the source context;
    // a source-free probe has neither.
    let has_sources = sources.is_some();
    syntax_trees_to_symbol_resolved_trees::pre_resolution::normalize_generic_data(
        syntax_trees_to_symbol_resolved_trees::pre_resolution::GenericDataRequest {
            syntax: syntax_trees,
            sources,
            top_level_bindings: if has_sources {
                source_scoped_top_level_bindings.to_vec()
            } else {
                Vec::new()
            },
            retained_base: if has_sources { retained_base } else { None },
        },
    )
}

pub(crate) fn resolve(
    syntax_trees: &syntax_trees::SyntaxTrees,
    sources: Option<Arc<source::SourceMap>>,
    source_scoped_top_level_bindings: &[symbols::SourceScopedTopLevelBinding],
) -> Result<symbol_resolved_trees::SymbolResolvedTrees, Vec<diagnostics::Diagnostic>> {
    match sources {
        Some(sources) => syntax_trees_to_symbol_resolved_trees::resolve(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest {
                syntax: syntax_trees,
                sources: Some(sources),
                top_level_bindings: source_scoped_top_level_bindings.to_vec(),
            },
        ),
        None => syntax_trees_to_symbol_resolved_trees::resolve(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(syntax_trees),
        ),
    }
}
