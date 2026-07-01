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
    /// Maps a match SUBJECT syntax expression handle to the name of the single
    /// hoisted temp for it. All arms of one enum-variant match share the same
    /// syntax subject handle (the parser reuses it across arms), so the first
    /// arm mints `let __hoist_N = <subject>` and the siblings reuse the name --
    /// keeping ONE shared subject so match exhaustiveness still groups the arms
    /// (`statement::hoist_membership_match_subject`). Keyed by the subject
    /// syntax handle's arena index (`Handle` is not `Hash`).
    match_subject_temps: std::collections::HashMap<u32, String>,
}

impl Lowerer {
    fn new(sources: Option<Arc<SourceMap>>) -> Self {
        Self {
            symbol_resolved_trees: SymbolResolvedTrees::default(),
            sources,
            hoist_counter: 0,
            match_subject_temps: std::collections::HashMap::new(),
        }
    }

    /// The shared hoist-temp name for a match subject syntax handle, if the first arm already
    /// minted one; otherwise records `name` as the temp for later arms and returns None.
    pub(crate) fn match_subject_temp(&mut self, subject_arena_index: u32) -> Option<String> {
        self.match_subject_temps.get(&subject_arena_index).cloned()
    }

    pub(crate) fn record_match_subject_temp(&mut self, subject_arena_index: u32, name: String) {
        self.match_subject_temps.insert(subject_arena_index, name);
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
