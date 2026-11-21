//! Identify compiler-owned data declarations in the admitted Build prelude.
use source_compiler::representations::source_state::TypedTrees;
use symbols::SymbolHandle;

pub(crate) fn is_exact_toolchain_build_prelude_data(
    typed: &TypedTrees,
    symbol: SymbolHandle,
    expected_name: &str,
) -> bool {
    typed.is_exact_toolchain_build_prelude_data(symbol, expected_name)
}

pub(super) fn has_exact_toolchain_build_facet(typed: &TypedTrees, name: &str) -> bool {
    typed.data_definitions().iter().any(|definition| {
        definition.name.as_str() == name && typed.symbols.is_build_prelude_symbol(definition.symbol)
    })
}
