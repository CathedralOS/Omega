use psi_arena::Arena;
use psi_symbol_resolved_trees::SymbolResolvedTrees;
use psi_symbols::{SymbolHandle, SymbolKind, SymbolTable};

use crate::symbols::expressions::assign_expression_table_symbols;
use crate::symbols::lookup::top_level_symbol;
use crate::symbols::scope::MachineScope;
use crate::symbols::top_level::{assign_machine_parameter_signature_symbols, next_child_of_kind};
use crate::symbols::type_references::{
    assign_proposition_family_argument_symbols,
    assign_type_reference_argument_symbols_with_constraints,
    assign_type_reference_symbol_with_locals_and_self_type_and_constraints,
};

pub(super) fn assign_machine_symbols(
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
    let tables = &mut program.tables;
    let type_constraints = &tables.types.constraints;
    let declarations = &mut tables.declarations;
    let expression_table = &mut tables.bodies.expressions;
    let data_type_parameters = &mut declarations.data_type_parameters;
    let data_members = &declarations.data_members;
    let machine_owned_data = &mut declarations.machine_owned_data;
    let machine_trait_conformances = &mut declarations.machine_trait_conformances;
    let machine_state_handles = &declarations.machine_state_handles;
    let machine_states = &mut declarations.machine_states;
    let state_parameters = &mut declarations.state_parameters;
    let child_type_references = &mut declarations.child_type_references;
    let psi_symbol_resolved_trees::SymbolResolvedRoots {
        data_definitions,
        machines,
        ..
    } = &mut program.roots;

    machines.for_each_mut(|machine| {
        let inherited_field_count = inherited_field_count(
            data_definitions.iter(),
            data_members,
            machine.attached_data.as_ref(),
        );
        machine.symbol = next_child_of_kind(root_children, symbols, SymbolKind::Machine);
        machine.attached_data_symbol = machine
            .attached_data
            .as_ref()
            .map(|attached| top_level_symbol(symbols, SymbolKind::Data, attached.as_str()))
            .unwrap_or_else(SymbolHandle::invalid);
        let machine_symbol = machine.symbol;
        let mut machine_children = symbols.child_handles(machine_symbol).into_iter().flatten();

        for type_parameter in data_type_parameters.span_mut_or_empty(machine.type_parameters) {
            if type_parameter.symbol.is_valid() {
                continue;
            }
            let kind = match type_parameter.kind {
                psi_symbol_resolved_trees::data::TypeParameterKind::Machine { .. } => {
                    SymbolKind::MachineParameter
                }
                _ => SymbolKind::TypeParameter,
            };
            type_parameter.symbol = next_child_of_kind(&mut machine_children, symbols, kind);
        }
        let local_type_parameters = data_type_parameters
            .span_or_empty(machine.type_parameters)
            .to_vec();

        for bound in &mut machine.conformance_bounds {
            if bound.binder_name.is_some() {
                bound.binder = Some(next_child_of_kind(
                    &mut machine_children,
                    symbols,
                    SymbolKind::ConformanceParameter,
                ));
            }
        }

        // MP1: machine-parameter contracts are real signature data, not
        // parser-only text. Resolve their parameter/result type references in
        // the declaring machine's generic/self context. Contract-parameter
        // symbols themselves join the modular-body checker in MP3; assigning
        // types here prevents representation loss meanwhile.
        for index in 0..machine.type_parameters.len() {
            let (parameter_symbol, kind) = {
                let parameter = &data_type_parameters.span_or_empty(machine.type_parameters)[index];
                (parameter.symbol, parameter.kind.clone())
            };
            let psi_symbol_resolved_trees::data::TypeParameterKind::Machine { mut contract } = kind
            else {
                continue;
            };
            if let Some(signature) = contract.structural_mut() {
                assign_machine_parameter_signature_symbols(
                    symbols,
                    data_type_parameters,
                    state_parameters,
                    child_type_references,
                    type_constraints,
                    signature,
                    parameter_symbol,
                    &local_type_parameters,
                    machine_symbol,
                );
            }
            data_type_parameters.span_mut_or_empty(machine.type_parameters)[index].kind =
                psi_symbol_resolved_trees::data::TypeParameterKind::Machine { contract };
        }

        for _ in 0..inherited_field_count {
            let _ = machine_children.next();
        }

        for owned_data in machine_owned_data.span_mut_or_empty(machine.owned_data) {
            owned_data.symbol =
                next_child_of_kind(&mut machine_children, symbols, SymbolKind::Field);
            assign_type_reference_symbol_with_locals_and_self_type_and_constraints(
                symbols,
                child_type_references,
                type_constraints,
                &local_type_parameters,
                machine_symbol,
                &mut owned_data.type_reference,
            );
            if owned_data.initial_value.is_valid() {
                assign_expression_table_symbols(
                    symbols,
                    &MachineScope {
                        symbol: machine_symbol,
                        type_parameters: &local_type_parameters,
                        attached_data: machine.attached_data.as_ref(),
                        owned_data: &[],
                        inherited_data_members: None,
                        data_definitions,
                        data_members,
                    },
                    &[],
                    SymbolHandle::invalid(),
                    expression_table,
                    child_type_references,
                    owned_data.initial_value,
                );
            }
        }

        for conformance in machine_trait_conformances.span_mut_or_empty(machine.satisfies) {
            conformance.symbol =
                top_level_symbol(symbols, SymbolKind::Trait, conformance.name.as_str());
            assign_type_reference_argument_symbols_with_constraints(
                symbols,
                child_type_references,
                type_constraints,
                &local_type_parameters,
                machine_symbol,
                conformance.arguments,
            );
            if let Some((_, proposition_slots)) = trait_proposition_slots
                .iter()
                .find(|(name, _)| name == conformance.name.as_str())
            {
                assign_proposition_family_argument_symbols(
                    symbols,
                    child_type_references,
                    &local_type_parameters,
                    conformance.arguments,
                    proposition_slots,
                );
            }
        }

        for bound in &mut machine.conformance_bounds {
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
                machine_symbol,
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

        for state in machine_state_handles
            .span_or_empty(machine.states)
            .iter()
            .copied()
        {
            let state = machine_states.get_mut(state);
            state.symbol = next_child_of_kind(&mut machine_children, symbols, SymbolKind::State);
            let state_symbol = state.symbol;
            let mut state_children = symbols.child_handles(state_symbol).into_iter().flatten();

            for parameter in state_parameters.span_mut_or_empty(state.parameters) {
                parameter.symbol =
                    next_child_of_kind(&mut state_children, symbols, SymbolKind::Parameter);
                assign_type_reference_symbol_with_locals_and_self_type_and_constraints(
                    symbols,
                    child_type_references,
                    type_constraints,
                    &local_type_parameters,
                    machine_symbol,
                    &mut parameter.type_reference,
                );
            }

            for statement in declarations
                .state_statements
                .span_mut_or_empty(state.statements)
            {
                if let psi_symbol_resolved_trees::statement::Statement::LocalData(local_data) =
                    statement
                {
                    local_data.symbol =
                        next_child_of_kind(&mut state_children, symbols, SymbolKind::Local);
                }
            }

            if let Some(return_type) = &mut state.return_type {
                assign_type_reference_symbol_with_locals_and_self_type_and_constraints(
                    symbols,
                    child_type_references,
                    type_constraints,
                    &local_type_parameters,
                    machine_symbol,
                    return_type,
                );
            }
        }
    });
}

fn inherited_field_count<'data>(
    data_definitions: impl IntoIterator<Item = &'data psi_symbol_resolved_trees::data::DataDefinition>,
    data_members: &Arena<psi_symbol_resolved_trees::data::DataMember>,
    attached_data: Option<&psi_symbol_resolved_trees::name::DiagnosticName>,
) -> usize {
    let Some(attached_data) = attached_data else {
        return 0;
    };

    data_definitions
        .into_iter()
        .find(|data_definition| data_definition.name == *attached_data)
        .map(|data_definition| {
            data_members
                .span_or_empty(data_definition.members)
                .iter()
                .filter(|member| {
                    matches!(
                        member,
                        psi_symbol_resolved_trees::data::DataMember::Field(_)
                    )
                })
                .count()
        })
        .unwrap_or(0)
}
