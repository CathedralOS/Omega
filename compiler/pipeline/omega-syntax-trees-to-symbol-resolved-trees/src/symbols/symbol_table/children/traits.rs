use omega_core::symbols::{SymbolHandle, SymbolKind, SymbolTableBuilder};
use omega_symbol_resolved_trees::SymbolResolvedTrees;

use crate::symbols::symbol_table::names::symbol_seed;

pub(in crate::symbols::symbol_table) fn insert_trait_symbol_children(
    builder: &mut SymbolTableBuilder,
    program: &SymbolResolvedTrees,
    trait_symbol: SymbolHandle,
    trait_definition: &omega_symbol_resolved_trees::trait_definition::TraitDefinition,
    has_sources: bool,
) {
    let trait_children = builder.insert_children(
        trait_symbol,
        program
            .trait_type_parameters(trait_definition)
            .iter()
            .map(|parameter| symbol_seed(SymbolKind::TypeParameter, &parameter.name, has_sources))
            .chain(
                program
                    .trait_machine_signatures(trait_definition.machines)
                    .iter()
                    .map(|machine| symbol_seed(SymbolKind::State, &machine.name, has_sources)),
            ),
    );

    let mut trait_children = SymbolTableBuilder::child_handles(trait_children);
    for _ in program.trait_type_parameters(trait_definition) {
        let _ = trait_children.next();
    }

    for (machine_symbol, machine) in trait_children.zip(
        program
            .trait_machine_signatures(trait_definition.machines)
            .iter(),
    ) {
        builder.insert_children(
            machine_symbol,
            program
                .state_parameters(machine.parameters)
                .iter()
                .map(|parameter| symbol_seed(SymbolKind::Parameter, &parameter.name, has_sources)),
        );
    }
}
