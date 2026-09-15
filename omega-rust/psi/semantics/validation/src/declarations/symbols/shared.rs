use symbols::SymbolHandle;
use typed_trees::TypedTrees;

pub(super) fn child_symbol(program: &TypedTrees, parent: SymbolHandle, name: &str) -> SymbolHandle {
    program
        .symbols
        .find_child_by_name(parent, name)
        .unwrap_or_else(SymbolHandle::invalid)
}
