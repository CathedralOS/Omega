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
    /// Per-lowering counter that mints unique names for synthetic `let`
    /// temporaries hoisted out of operand-position indexed reads (see
    /// `statement::hoist_indexed_operands`). `__hoist_` prefixed so the
    /// generated names cannot collide with source identifiers.
    hoist_counter: u32,
}

impl Lowerer {
    fn new(sources: Option<Arc<SourceMap>>) -> Self {
        Self {
            symbol_resolved_trees: SymbolResolvedTrees::default(),
            sources,
            hoist_counter: 0,
        }
    }

    pub(crate) fn next_hoist_name(&mut self) -> String {
        let name = format!("__hoist_{}", self.hoist_counter);
        self.hoist_counter += 1;
        name
    }

    pub(crate) fn finish(mut self) -> Result<SymbolResolvedTrees, Diagnostic> {
        crate::symbols::assign_symbols(&mut self.symbol_resolved_trees, self.sources);
        self.symbol_resolved_trees.rebuild_tables();
        let SymbolResolvedTrees {
            roots,
            tables,
            symbols,
        } = self.symbol_resolved_trees;

        Ok(SymbolResolvedTrees::with_roots(roots, tables, symbols))
    }
}

#[cfg(test)]
mod tests;
