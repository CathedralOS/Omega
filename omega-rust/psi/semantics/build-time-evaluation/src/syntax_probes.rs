//! Resolve and normalize temporary syntax with the originating loader's custody.

use std::sync::Arc;

pub(crate) fn normalize_generic_data(
    syntax_trees: syntax_trees::SyntaxTrees,
    sources: Option<Arc<source::SourceMap>>,
    source_scoped_top_level_bindings: &[symbols::SourceScopedTopLevelBinding],
    retained_base: Option<&symbol_resolved_trees::SymbolResolvedTrees>,
) -> Result<syntax_trees::SyntaxTrees, Vec<diagnostics::Diagnostic>> {
    match sources {
        Some(sources) => {
            syntax_trees_to_symbol_resolved_trees::normalize_generic_data_with_retained_base(
                syntax_trees,
                sources,
                source_scoped_top_level_bindings.to_vec(),
                retained_base,
            )
        }
        None => syntax_trees_to_symbol_resolved_trees::normalize_generic_data(syntax_trees),
    }
}

pub(crate) fn resolve(
    syntax_trees: &syntax_trees::SyntaxTrees,
    sources: Option<Arc<source::SourceMap>>,
    source_scoped_top_level_bindings: &[symbols::SourceScopedTopLevelBinding],
) -> Result<symbol_resolved_trees::SymbolResolvedTrees, Vec<diagnostics::Diagnostic>> {
    match sources {
        Some(sources) => syntax_trees_to_symbol_resolved_trees::lower_syntax_trees_with_sources_and_top_level_bindings(
            syntax_trees,
            sources,
            source_scoped_top_level_bindings.to_vec(),
        ),
        None => syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(syntax_trees),
    }
}
