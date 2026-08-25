use psi_symbol_resolved_trees::SymbolResolvedTrees;
use psi_symbols::{SymbolHandle, SymbolKind, SymbolTable};

use crate::symbols::lookup::top_level_symbol;
use crate::symbols::top_level::{
    assign_machine_parameter_signature_symbols, assign_proposition_parameter_signature_symbols,
    next_child_of_kind,
};
use crate::symbols::type_references::{
    assign_proposition_family_argument_symbols,
    assign_type_reference_argument_symbols_with_constraints,
    assign_type_reference_symbol_with_locals_and_self_type_and_constraints,
};

pub(super) fn assign_trait_symbols(
    program: &mut SymbolResolvedTrees,
    symbols: &SymbolTable,
    root_children: &mut impl Iterator<Item = SymbolHandle>,
) {
    let trait_proposition_slots = program
        .roots
        .traits
        .iter()
        .map(|trait_definition| {
            (
                trait_definition.name.as_str().to_owned(),
                program
                    .tables
                    .declarations
                    .data_type_parameters
                    .span_or_empty(trait_definition.type_parameters)
                    .iter()
                    .map(|parameter| {
                        matches!(
                            parameter.kind,
                            psi_symbol_resolved_trees::data::TypeParameterKind::Proposition { .. }
                        )
                    })
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<Vec<_>>();
    let type_constraints = &program.tables.types.constraints;
    let declarations = &mut program.tables.declarations;
    let data_type_parameters = &mut declarations.data_type_parameters;
    let trait_requirements = &mut declarations.trait_requirements;
    let trait_machine_signatures = &mut declarations.trait_machine_signatures;
    let state_parameters = &mut declarations.state_parameters;
    let child_type_references = &mut declarations.child_type_references;
    program.roots.traits.for_each_mut(|trait_definition| {
        trait_definition.symbol = next_child_of_kind(root_children, symbols, SymbolKind::Trait);
        let trait_symbol = trait_definition.symbol;
        let mut trait_children = symbols.child_handles(trait_symbol).into_iter().flatten();

        for type_parameter in
            data_type_parameters.span_mut_or_empty(trait_definition.type_parameters)
        {
            let kind = match type_parameter.kind {
                psi_symbol_resolved_trees::data::TypeParameterKind::Proposition { .. } => {
                    SymbolKind::PropositionParameter
                }
                psi_symbol_resolved_trees::data::TypeParameterKind::Machine { .. } => {
                    SymbolKind::MachineParameter
                }
                _ => SymbolKind::TypeParameter,
            };
            type_parameter.symbol = next_child_of_kind(&mut trait_children, symbols, kind);
        }
        let local_type_parameters = data_type_parameters
            .span_or_empty(trait_definition.type_parameters)
            .to_vec();

        for bound in &mut trait_definition.conformance_bounds {
            if bound.binder_name.is_some() {
                bound.binder = Some(next_child_of_kind(
                    &mut trait_children,
                    symbols,
                    SymbolKind::ConformanceParameter,
                ));
            }
        }

        for index in 0..trait_definition.type_parameters.len() {
            let (parameter_symbol, kind) = {
                let parameter =
                    &data_type_parameters.span_or_empty(trait_definition.type_parameters)[index];
                (parameter.symbol, parameter.kind.clone())
            };
            let resolved_kind = match kind {
                psi_symbol_resolved_trees::data::TypeParameterKind::Const {
                    mut type_reference,
                } => {
                    assign_type_reference_symbol_with_locals_and_self_type_and_constraints(
                        symbols,
                        child_type_references,
                        type_constraints,
                        &local_type_parameters,
                        trait_symbol,
                        &mut type_reference,
                    );
                    psi_symbol_resolved_trees::data::TypeParameterKind::Const { type_reference }
                }
                psi_symbol_resolved_trees::data::TypeParameterKind::Proposition {
                    mut contract,
                } => {
                    assign_proposition_parameter_signature_symbols(
                        symbols,
                        state_parameters,
                        child_type_references,
                        type_constraints,
                        &mut contract,
                        parameter_symbol,
                        &local_type_parameters,
                        trait_symbol,
                    );
                    psi_symbol_resolved_trees::data::TypeParameterKind::Proposition { contract }
                }
                other => other,
            };
            data_type_parameters.span_mut_or_empty(trait_definition.type_parameters)[index].kind =
                resolved_kind;
        }

        for bound in &mut trait_definition.conformance_bounds {
            bound.subject = local_type_parameters
                .iter()
                .find(|parameter| parameter.name == bound.subject_name)
                .map(|parameter| parameter.symbol)
                .unwrap_or_else(SymbolHandle::invalid);
            if let Some(conformance_name) = &bound.conformance_name {
                bound.carrier =
                    top_level_symbol(symbols, SymbolKind::Data, bound.carrier_name.as_str());
                let selected =
                    top_level_symbol(symbols, SymbolKind::Conformance, conformance_name.as_str());
                bound.conformance = selected.is_valid().then_some(selected);
            } else {
                bound.carrier =
                    top_level_symbol(symbols, SymbolKind::Trait, bound.carrier_name.as_str());
            }
            assign_type_reference_argument_symbols_with_constraints(
                symbols,
                child_type_references,
                type_constraints,
                &local_type_parameters,
                trait_symbol,
                bound.arguments,
            );
            if bound.conformance_name.is_none()
                && let Some((_, proposition_slots)) = trait_proposition_slots
                    .iter()
                    .find(|(name, _)| name == bound.carrier_name.as_str())
            {
                assign_proposition_family_argument_symbols(
                    symbols,
                    child_type_references,
                    &local_type_parameters,
                    bound.arguments,
                    proposition_slots,
                );
            }
        }

        for requirement in trait_requirements.span_mut_or_empty(trait_definition.requires) {
            requirement.symbol =
                top_level_symbol(symbols, SymbolKind::Trait, requirement.name.as_str());
            assign_type_reference_argument_symbols_with_constraints(
                symbols,
                child_type_references,
                type_constraints,
                &local_type_parameters,
                trait_symbol,
                requirement.arguments,
            );
            if let Some((_, proposition_slots)) = trait_proposition_slots
                .iter()
                .find(|(name, _)| name == requirement.name.as_str())
            {
                assign_proposition_family_argument_symbols(
                    symbols,
                    child_type_references,
                    &local_type_parameters,
                    requirement.arguments,
                    proposition_slots,
                );
            }
        }

        for machine in trait_machine_signatures.span_mut_or_empty(trait_definition.machines) {
            machine.symbol = next_child_of_kind(&mut trait_children, symbols, SymbolKind::State);
            let machine_symbol = machine.symbol;
            assign_machine_parameter_signature_symbols(
                symbols,
                data_type_parameters,
                state_parameters,
                child_type_references,
                type_constraints,
                machine,
                machine_symbol,
                &local_type_parameters,
                trait_symbol,
            );
        }
    });
}
