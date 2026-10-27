//! Checking context is not an executable declaration selection.
//!
//! A private preparation may retain generic templates to compute ordinary
//! contract identities. This ceiling continues to name only the original
//! executable selection. Checking dependencies and specialization ancestry do
//! not extend this ceiling; an exact new demand must be admitted by its owner.

use symbols::SymbolHandle;
use typed_trees::references::DeclarationSelection;

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct ExecutionScope {
    declarations: DeclarationSelection,
}

impl ExecutionScope {
    pub(crate) fn new(declarations: DeclarationSelection) -> Self {
        Self { declarations }
    }

    pub(crate) fn unrestricted() -> Self {
        Self::new(DeclarationSelection::all())
    }

    /// Only the owner which minted this exact private probe may extend scope.
    /// Its callees still undergo the independent closure check.
    pub(crate) fn with_probe(&self, symbol: SymbolHandle) -> Self {
        let mut scope = self.clone();
        scope.declarations.include(symbol);
        scope
    }

    pub(crate) fn permits(&self, symbol: SymbolHandle) -> bool {
        self.declarations.contains(symbol)
    }
}
