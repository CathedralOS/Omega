use psi_facts::ScalarValue;
use psi_symbols::SymbolHandle;

pub(crate) trait ScalarValueSource {
    fn binding(&mut self, position: usize) -> Option<ScalarValue>;

    fn storage(&mut self, _symbol: SymbolHandle) -> Option<ScalarValue> {
        None
    }
}

impl<Resolve: FnMut(usize) -> Option<ScalarValue>> ScalarValueSource for Resolve {
    fn binding(&mut self, position: usize) -> Option<ScalarValue> {
        self(position)
    }
}

/// Exact immutable binding identities and separately named current storage
/// reads. Both resolve only against the caller's facts at this program point.
pub(crate) struct BoundScalarValues<'a, Resolve> {
    pub symbols: &'a [SymbolHandle],
    pub value_at_symbol: Resolve,
}

impl<Resolve: FnMut(SymbolHandle) -> Option<ScalarValue>> ScalarValueSource
    for BoundScalarValues<'_, Resolve>
{
    fn binding(&mut self, position: usize) -> Option<ScalarValue> {
        (self.value_at_symbol)(*self.symbols.get(position)?)
    }

    fn storage(&mut self, symbol: SymbolHandle) -> Option<ScalarValue> {
        (self.value_at_symbol)(symbol)
    }
}
