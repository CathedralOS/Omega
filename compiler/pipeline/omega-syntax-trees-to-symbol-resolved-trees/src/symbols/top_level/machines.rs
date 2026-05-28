use omega_core::arena::Arena;
use omega_core::symbols::{SymbolHandle, SymbolKind, SymbolTable};
use omega_symbol_resolved_trees::SymbolResolvedTrees;

use crate::symbols::expressions::assign_expression_table_symbols;
use crate::symbols::lookup::top_level_symbol;
use crate::symbols::scope::MachineScope;
use crate::symbols::top_level::next_child_of_kind;
use crate::symbols::type_references::assign_type_reference_symbol_with_self_type;

pub(super) fn assign_machine_symbols(
    program: &mut SymbolResolvedTrees,
    symbols: &SymbolTable,
    root_children: &mut impl Iterator<Item = SymbolHandle>,
) {
    let tables = &mut program.tables;
    let declarations = &mut tables.declarations;
    let expression_table = &mut tables.bodies.expressions;
    let data_members = &declarations.data_members;
    let machine_contained_objects = &mut declarations.machine_contained_objects;
    let machine_owned_data = &mut declarations.machine_owned_data;
    let machine_trait_conformances = &mut declarations.machine_trait_conformances;
    let machine_state_handles = &declarations.machine_state_handles;
    let machine_states = &mut declarations.machine_states;
    let state_parameters = &mut declarations.state_parameters;
    let child_type_references = &mut declarations.child_type_references;
    let omega_symbol_resolved_trees::SymbolResolvedRoots {
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
        let machine_symbol = machine.symbol;
        let mut machine_children = symbols.child_handles(machine_symbol).into_iter().flatten();

        for _ in 0..inherited_field_count {
            let _ = machine_children.next();
        }

        for contained_object in machine_contained_objects.span_mut_or_empty(machine.contains) {
            contained_object.symbol =
                next_child_of_kind(&mut machine_children, symbols, SymbolKind::Object);
            contained_object.type_symbol = top_level_symbol(
                symbols,
                SymbolKind::Machine,
                contained_object.type_name.as_str(),
            );
        }

        for owned_data in machine_owned_data.span_mut_or_empty(machine.owned_data) {
            owned_data.symbol =
                next_child_of_kind(&mut machine_children, symbols, SymbolKind::Field);
            assign_type_reference_symbol_with_self_type(
                symbols,
                child_type_references,
                machine_symbol,
                &mut owned_data.type_reference,
            );
            if owned_data.initial_value.is_valid() {
                assign_expression_table_symbols(
                    symbols,
                    &MachineScope {
                        symbol: machine_symbol,
                        attached_data: machine.attached_data.as_ref(),
                        owned_data: &[],
                        inherited_data_members: None,
                        contains: &[],
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
                assign_type_reference_symbol_with_self_type(
                    symbols,
                    child_type_references,
                    machine_symbol,
                    &mut parameter.type_reference,
                );
            }

            for statement in declarations
                .state_statements
                .span_mut_or_empty(state.statements)
            {
                if let omega_symbol_resolved_trees::statement::Statement::LocalData(local_data) =
                    statement
                {
                    local_data.symbol =
                        next_child_of_kind(&mut state_children, symbols, SymbolKind::Local);
                }
            }

            if let Some(return_type) = &mut state.return_type {
                assign_type_reference_symbol_with_self_type(
                    symbols,
                    child_type_references,
                    machine_symbol,
                    return_type,
                );
            }
        }
    });
}

fn inherited_field_count<'data>(
    data_definitions: impl IntoIterator<Item = &'data omega_symbol_resolved_trees::data::DataDefinition>,
    data_members: &Arena<omega_symbol_resolved_trees::data::DataMember>,
    attached_data: Option<&omega_symbol_resolved_trees::name::DiagnosticName>,
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
                        omega_symbol_resolved_trees::data::DataMember::Field(_)
                    )
                })
                .count()
        })
        .unwrap_or(0)
}
