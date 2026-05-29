use omega_core::symbols::{SymbolHandle, SymbolKind, SymbolTableBuilder};
use omega_symbol_resolved_trees::SymbolResolvedTrees;

use crate::symbols::symbol_table::names::symbol_seed;

pub(in crate::symbols::symbol_table) fn insert_platform_symbol_children(
    builder: &mut SymbolTableBuilder,
    program: &SymbolResolvedTrees,
    platform_symbol: SymbolHandle,
    platform: &omega_symbol_resolved_trees::platform::Platform,
    has_sources: bool,
) {
    let platform_children = builder.insert_children(
        platform_symbol,
        program
            .platform_state_signatures(platform.states)
            .iter()
            .map(|state| symbol_seed(SymbolKind::State, &state.name, has_sources)),
    );

    for (state_symbol, state) in SymbolTableBuilder::child_handles(platform_children)
        .zip(program.platform_state_signatures(platform.states).iter())
    {
        builder.insert_children(
            state_symbol,
            program
                .state_parameters(state.parameters)
                .iter()
                .map(|parameter| symbol_seed(SymbolKind::Parameter, &parameter.name, has_sources)),
        );
    }
}
