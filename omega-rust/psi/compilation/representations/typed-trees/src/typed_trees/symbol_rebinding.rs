//! Borrowed correspondences for cloning lexical content and executable bodies.
//! A specialization changes the execution entry without changing the authored
//! declaration to which a retained static operand applies its complete tuple.

use symbols::SymbolHandle;

type SymbolPairs = [(SymbolHandle, SymbolHandle)];

#[derive(Clone, Copy)]
pub struct SymbolRebinding<'scope> {
    execution: &'scope SymbolPairs,
    authored: &'scope SymbolPairs,
}

impl<'scope> SymbolRebinding<'scope> {
    pub fn new(execution: &'scope SymbolPairs, authored: &'scope SymbolPairs) -> Self {
        Self {
            execution,
            authored,
        }
    }

    pub(crate) fn execution(self) -> &'scope SymbolPairs {
        self.execution
    }

    /// Correspondence for declaration-valued operands. Executable entry
    /// specialization is excluded; lexical formals and captures still rebase.
    pub fn authored(self) -> &'scope SymbolPairs {
        self.authored
    }
}

impl<'scope> From<&'scope SymbolPairs> for SymbolRebinding<'scope> {
    fn from(symbols: &'scope SymbolPairs) -> Self {
        Self::new(symbols, symbols)
    }
}

impl<'scope> From<&'scope Vec<(SymbolHandle, SymbolHandle)>> for SymbolRebinding<'scope> {
    fn from(symbols: &'scope Vec<(SymbolHandle, SymbolHandle)>) -> Self {
        Self::from(symbols.as_slice())
    }
}

impl<'scope, const N: usize> From<&'scope [(SymbolHandle, SymbolHandle); N]>
    for SymbolRebinding<'scope>
{
    fn from(symbols: &'scope [(SymbolHandle, SymbolHandle); N]) -> Self {
        Self::from(symbols.as_slice())
    }
}

impl std::ops::Deref for SymbolRebinding<'_> {
    type Target = SymbolPairs;

    fn deref(&self) -> &Self::Target {
        self.execution
    }
}
