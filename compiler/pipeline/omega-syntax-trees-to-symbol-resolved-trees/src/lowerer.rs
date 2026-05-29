use crate::item::lower_item;
use omega_core::diagnostics::Diagnostic;
use omega_core::source::SourceMap;
use omega_symbol_resolved_trees::SymbolResolvedTrees;
use omega_syntax_trees::SyntaxTrees;
use std::sync::Arc;

pub fn lower_syntax_trees(syntax_trees: &SyntaxTrees) -> Result<SymbolResolvedTrees, Diagnostic> {
    lower_syntax_trees_with_optional_sources(syntax_trees, None)
}

pub fn lower_syntax_trees_with_sources(
    syntax_trees: &SyntaxTrees,
    sources: Arc<SourceMap>,
) -> Result<SymbolResolvedTrees, Diagnostic> {
    lower_syntax_trees_with_optional_sources(syntax_trees, Some(sources))
}

fn lower_syntax_trees_with_optional_sources(
    syntax_trees: &SyntaxTrees,
    sources: Option<Arc<SourceMap>>,
) -> Result<SymbolResolvedTrees, Diagnostic> {
    let mut lowerer = Lowerer::new(sources);

    for item in syntax_trees.root_items() {
        lower_item(&mut lowerer, syntax_trees, item)?;
    }

    lowerer.finish()
}

pub(crate) struct Lowerer {
    pub(crate) symbol_resolved_trees: SymbolResolvedTrees,
    sources: Option<Arc<SourceMap>>,
}

impl Lowerer {
    fn new(sources: Option<Arc<SourceMap>>) -> Self {
        Self {
            symbol_resolved_trees: SymbolResolvedTrees::default(),
            sources,
        }
    }

    pub(crate) fn finish(mut self) -> Result<SymbolResolvedTrees, Diagnostic> {
        crate::symbols::assign_symbols(&mut self.symbol_resolved_trees, self.sources);
        self.symbol_resolved_trees.rebuild_tables();
        Ok(self.symbol_resolved_trees)
    }
}

#[cfg(test)]
mod tests;
