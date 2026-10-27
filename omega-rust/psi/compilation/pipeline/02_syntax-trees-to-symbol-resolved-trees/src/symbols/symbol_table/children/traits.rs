use symbol_resolved_trees::SymbolResolvedTrees;
use symbols::{SymbolHandle, SymbolKind, SymbolTableAppender, SymbolTableBuilder};

use crate::symbols::symbol_table::names::{parameter_seeds, symbol_seed};

pub(in crate::symbols::symbol_table) fn insert_trait_symbol_children(
    builder: &mut impl SymbolTableAppender,
    program: &SymbolResolvedTrees,
    trait_symbol: SymbolHandle,
    trait_definition: &symbol_resolved_trees::trait_definition::TraitDefinition,
    has_sources: bool,
) {
    let trait_children = builder.insert_children(
        trait_symbol,
        program
            .trait_type_parameters(trait_definition)
            .iter()
            .map(|parameter| {
                let kind = match parameter.kind {
                    symbol_resolved_trees::data::TypeParameterKind::Implementation => {
                        SymbolKind::ConformanceParameter
                    }
                    symbol_resolved_trees::data::TypeParameterKind::Machine { .. } => {
                        SymbolKind::MachineParameter
                    }
                    symbol_resolved_trees::data::TypeParameterKind::Proposition { .. } => {
                        SymbolKind::PropositionParameter
                    }
                    _ => SymbolKind::TypeParameter,
                };
                symbol_seed(kind, &parameter.name, has_sources)
            })
            .chain(
                program
                    .trait_machine_signatures(trait_definition.machines)
                    .iter()
                    .map(|machine| symbol_seed(SymbolKind::State, &machine.name, has_sources)),
            ),
    );

    let mut trait_children = SymbolTableBuilder::child_handles(trait_children);
    for parameter in program.trait_type_parameters(trait_definition) {
        let parameter_symbol = trait_children.next();
        if let (
            Some(parameter_symbol),
            symbol_resolved_trees::data::TypeParameterKind::Proposition { contract },
        ) = (parameter_symbol, &parameter.kind)
        {
            builder.insert_children(
                parameter_symbol,
                parameter_seeds(program.state_parameters(contract.parameters), has_sources),
            );
        }
    }

    for (machine_symbol, machine) in trait_children.zip(
        program
            .trait_machine_signatures(trait_definition.machines)
            .iter(),
    ) {
        super::insert_machine_parameter_signature_children(
            builder,
            program,
            machine_symbol,
            machine,
            has_sources,
        );
    }
}
