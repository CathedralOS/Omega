use omega_core::symbols::{SymbolHandle, SymbolKind, SymbolTable};
use omega_symbol_resolved_trees::SymbolResolvedTrees;

use crate::symbols::top_level::next_child_of_kind;
use crate::symbols::type_references::assign_type_reference_symbol_with_self_type;

pub(super) fn assign_platform_symbols(
    program: &mut SymbolResolvedTrees,
    symbols: &SymbolTable,
    root_children: &mut impl Iterator<Item = SymbolHandle>,
) {
    let declarations = &mut program.tables.declarations;
    let platform_state_signatures = &mut declarations.platform_state_signatures;
    let state_parameters = &mut declarations.state_parameters;
    let child_type_references = &mut declarations.child_type_references;
    program.roots.platforms.for_each_mut(|platform| {
        platform.symbol = next_child_of_kind(root_children, symbols, SymbolKind::Platform);
        let platform_symbol = platform.symbol;
        let mut platform_children = symbols.child_handles(platform_symbol).into_iter().flatten();

        for state in platform_state_signatures.span_mut_or_empty(platform.states) {
            state.symbol = next_child_of_kind(&mut platform_children, symbols, SymbolKind::State);
            let state_symbol = state.symbol;
            let mut state_children = symbols.child_handles(state_symbol).into_iter().flatten();

            for parameter in state_parameters.span_mut_or_empty(state.parameters) {
                parameter.symbol =
                    next_child_of_kind(&mut state_children, symbols, SymbolKind::Parameter);
                assign_type_reference_symbol_with_self_type(
                    symbols,
                    child_type_references,
                    platform_symbol,
                    &mut parameter.type_reference,
                );
            }

            if let Some(return_type) = &mut state.return_type {
                assign_type_reference_symbol_with_self_type(
                    symbols,
                    child_type_references,
                    platform_symbol,
                    return_type,
                );
            }
        }
    });
}
