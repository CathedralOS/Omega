//! Purpose-specific declaration custody for provider planning.
//!
//! Build and product may retain separate checked instances of identical source.
//! This borrowed view selects the declaration frontier for schema derivation and
//! replay; it grants no admission or execution authority and creates no new IR.
//! Target selection remains an explicit operation coordinate, retained and
//! validated by evaluated binding tables and provider-plan derivation requests.

use source::DependencyScope;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;

/// The typed program and dependency purpose whose declarations a planning
/// operation may consume. Callers must select the purpose explicitly.
#[derive(Copy, Clone)]
pub struct ProviderPlanningContext<'p> {
    pub typed: &'p TypedTrees,
    pub scope: DependencyScope,
}

impl<'p> ProviderPlanningContext<'p> {
    /// Borrow one explicit declaration frontier without selecting a target or
    /// establishing execution authority.
    pub const fn new(typed: &'p TypedTrees, scope: DependencyScope) -> Self {
        Self { typed, scope }
    }

    /// Test retained source provenance without substituting an equal package or
    /// signature from the other purpose. Generated symbols follow their authored
    /// provenance; source-free declarations retain their existing unscoped behavior.
    pub fn contains(self, symbol: SymbolHandle) -> bool {
        self.typed
            .symbols
            .symbol_provenance_source_span(symbol)
            .and_then(|span| self.typed.symbols.source_file(span))
            .is_none_or(|file| file.dependency_scope == self.scope)
    }
}
