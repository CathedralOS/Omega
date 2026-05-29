mod data;
mod domains;
mod machines;
mod operators;
mod platforms;
mod traits;

use omega_core::symbols::{
    SymbolHandle, SymbolKind, SymbolTable, builtin_function_symbols, builtin_type_symbols,
};
use omega_symbol_resolved_trees::SymbolResolvedTrees;

use super::top_level::data::assign_data_symbols;
use super::top_level::domains::assign_domain_symbols;
use super::top_level::machines::assign_machine_symbols;
use super::top_level::operators::assign_root_operator_symbols;
use super::top_level::platforms::assign_platform_symbols;
use super::top_level::traits::assign_trait_symbols;

pub(super) fn assign_top_level_symbols(program: &mut SymbolResolvedTrees, symbols: &SymbolTable) {
    let mut root_children = symbols.child_handles(symbols.root()).into_iter().flatten();

    for _ in 0..builtin_type_symbols().len() {
        let _ = root_children.next();
    }
    for _ in 0..builtin_function_symbols().len() {
        let _ = root_children.next();
    }

    program.invariant_definitions.for_each_mut(|invariant| {
        invariant.symbol = next_child_of_kind(&mut root_children, symbols, SymbolKind::Invariant);
    });

    assign_domain_symbols(program, symbols, &mut root_children);
    assign_data_symbols(program, symbols, &mut root_children);
    assign_machine_symbols(program, symbols, &mut root_children);
    assign_root_operator_symbols(program, symbols, &mut root_children);
    assign_platform_symbols(program, symbols, &mut root_children);
    assign_trait_symbols(program, symbols, &mut root_children);
}

pub(super) fn next_child_of_kind(
    children: &mut impl Iterator<Item = SymbolHandle>,
    symbols: &SymbolTable,
    kind: SymbolKind,
) -> SymbolHandle {
    let Some(child) = children.next() else {
        return SymbolHandle::invalid();
    };

    if symbols.get(child).kind == kind {
        child
    } else {
        SymbolHandle::invalid()
    }
}
