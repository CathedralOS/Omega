use omega_core::symbols::{SymbolHandle, SymbolKind, SymbolTable};
use omega_symbol_resolved_trees::SymbolResolvedTrees;

use crate::symbols::lookup::top_level_symbol;
use crate::symbols::top_level::next_child_of_kind;
use crate::symbols::type_references::assign_type_reference_symbol_with_self_type;

pub(super) fn assign_trait_symbols(
    program: &mut SymbolResolvedTrees,
    symbols: &SymbolTable,
    root_children: &mut impl Iterator<Item = SymbolHandle>,
) {
    let declarations = &mut program.tables.declarations;
    let trait_requirements = &mut declarations.trait_requirements;
    let trait_machine_signatures = &mut declarations.trait_machine_signatures;
    let state_parameters = &mut declarations.state_parameters;
    let child_type_references = &mut declarations.child_type_references;
    program.roots.traits.for_each_mut(|trait_definition| {
        trait_definition.symbol = next_child_of_kind(root_children, symbols, SymbolKind::Trait);
        let trait_symbol = trait_definition.symbol;
        let mut trait_children = symbols.child_handles(trait_symbol).into_iter().flatten();

        for requirement in trait_requirements.span_mut_or_empty(trait_definition.requires) {
            requirement.symbol =
                top_level_symbol(symbols, SymbolKind::Trait, requirement.name.as_str());
        }

        for machine in trait_machine_signatures.span_mut_or_empty(trait_definition.machines) {
            machine.symbol = next_child_of_kind(&mut trait_children, symbols, SymbolKind::State);
            let machine_symbol = machine.symbol;
            let mut machine_children = symbols.child_handles(machine_symbol).into_iter().flatten();

            for parameter in state_parameters.span_mut_or_empty(machine.parameters) {
                parameter.symbol =
                    next_child_of_kind(&mut machine_children, symbols, SymbolKind::Parameter);
                assign_type_reference_symbol_with_self_type(
                    symbols,
                    child_type_references,
                    trait_symbol,
                    &mut parameter.type_reference,
                );
            }

            if let Some(return_type) = &mut machine.return_type {
                assign_type_reference_symbol_with_self_type(
                    symbols,
                    child_type_references,
                    trait_symbol,
                    return_type,
                );
            }
        }
    });
}
