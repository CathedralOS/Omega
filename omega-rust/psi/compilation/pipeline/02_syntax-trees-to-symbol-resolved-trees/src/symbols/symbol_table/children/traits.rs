use symbol_resolved_trees::SymbolResolvedTrees;
use symbols::{SymbolHandle, SymbolKind, SymbolTableAppender, SymbolTableBuilder};

use crate::symbols::symbol_table::names::{parameter_seeds, symbol_seed};
use symbol_resolved_trees::data::TypeParameterKind;
use symbol_resolved_trees::trait_definition::TraitDefinition;

pub(in crate::symbols::symbol_table) fn insert_trait_symbol_children(
    builder: &mut impl SymbolTableAppender,
    program: &SymbolResolvedTrees,
    trait_symbol: SymbolHandle,
    trait_definition: &TraitDefinition,
    has_sources: bool,
    sources: Option<&source::SourceMap>,
) {
    let trait_children = builder.insert_children(
        trait_symbol,
        program
            .trait_type_parameters(trait_definition)
            .iter()
            .map(|parameter| {
                let kind = match parameter.kind {
                    TypeParameterKind::Implementation => SymbolKind::ConformanceParameter,
                    TypeParameterKind::Machine { .. } => SymbolKind::MachineParameter,
                    TypeParameterKind::Proposition { .. } => SymbolKind::PropositionParameter,
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
        if let (Some(parameter_symbol), TypeParameterKind::Proposition { contract }) =
            (parameter_symbol, &parameter.kind)
        {
            builder.insert_children(
                parameter_symbol,
                parameter_seeds(program.state_parameters(contract.parameters), has_sources),
            );
        }
        if let Some(parameter_symbol) = parameter_symbol
            && let Some(bound) = trait_definition
                .conformance_bounds
                .iter()
                .find(|bound| bound.binder_name == parameter.name)
            && let Some(declaration) = super::machines::select_visible_trait_definition(
                program,
                &bound.trait_name,
                sources,
            )
        {
            // A trait law owns the same selected requirement namespace as a
            // machine body. Its evidence parameter is never a runtime value.
            let mut requirements = Vec::new();
            super::machines::collect_evidence_requirement_closure(
                program,
                declaration,
                sources,
                &mut Vec::new(),
                &mut requirements,
            );
            builder.insert_children(
                parameter_symbol,
                requirements.into_iter().map(|requirement| {
                    symbol_seed(SymbolKind::State, &requirement.name, has_sources)
                }),
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
