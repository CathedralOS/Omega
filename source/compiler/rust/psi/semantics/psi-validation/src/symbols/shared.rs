use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;

pub(super) fn top_level_symbol(program: &TypedTrees, name: &str) -> SymbolHandle {
    child_symbol(program, program.symbols.root(), name)
}

pub(super) fn child_symbol(program: &TypedTrees, parent: SymbolHandle, name: &str) -> SymbolHandle {
    program
        .symbols
        .find_child_by_name(parent, name)
        .unwrap_or_else(SymbolHandle::invalid)
}
