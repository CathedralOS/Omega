use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::trait_definition::TraitDefinition;

pub(super) fn trait_definition_by_symbol(
    program: &TypedTrees,
    symbol: SymbolHandle,
) -> Option<&TraitDefinition> {
    if !symbol.is_valid() {
        return None;
    }

    program
        .traits()
        .iter()
        .find(|trait_definition| trait_definition.symbol == symbol)
}
