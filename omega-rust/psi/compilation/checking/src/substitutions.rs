//! Lookup over `(parameter, actual)` substitution bindings shared by the
//! value-custody and machine-call validators.

use symbols::SymbolHandle;

/// The binding a substitution list assigns `symbol`: later entries shadow
/// earlier ones, so the search runs the list in reverse.
pub fn actual_for<T: Clone>(substitutions: &[(SymbolHandle, T)], symbol: SymbolHandle) -> Option<T> {
    substitutions
        .iter()
        .rev()
        .find(|(parameter, _)| *parameter == symbol)
        .map(|(_, actual)| actual.clone())
}
