use omega_core::arena::Arena;
use omega_core::symbols::{
    SymbolHandle, SymbolKind, SymbolTable, builtin_function_symbols, builtin_type_symbols,
};
use omega_symbol_resolved_trees::SymbolResolvedTrees;

use super::assign_expression_table_symbols;
use super::lookup::top_level_symbol;
use super::scope::MachineScope;
use super::type_references::{
    assign_type_reference_symbol_with_locals, assign_type_reference_symbol_with_self_type,
};

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

    {
        let roots = &mut program.roots;
        let declarations = &mut program.tables.declarations;
        let operator_definitions = &mut declarations.operator_definitions;
        let data_type_parameters = &mut declarations.data_type_parameters;
        let state_parameters = &mut declarations.state_parameters;
        let child_type_references = &mut declarations.child_type_references;
        roots.domain_definitions.for_each_mut(|domain| {
            domain.symbol = next_child_of_kind(&mut root_children, symbols, SymbolKind::Domain);
            let mut domain_children = symbols.child_handles(domain.symbol).into_iter().flatten();
            for operator in operator_definitions.span_mut_or_empty(domain.operators) {
                assign_operator_symbols(
                    symbols,
                    &mut domain_children,
                    data_type_parameters,
                    state_parameters,
                    child_type_references,
                    operator,
                );
            }
        });
    }

    let data_type_parameters = &mut program.tables.declarations.data_type_parameters;
    let data_members = &mut program.tables.declarations.data_members;
    program
        .roots
        .data_definitions
        .for_each_mut(|data_definition| {
            data_definition.symbol =
                next_child_of_kind(&mut root_children, symbols, SymbolKind::Data);
            let data_symbol = data_definition.symbol;
            let mut data_children = symbols.child_handles(data_symbol).into_iter().flatten();

            for type_parameter in
                data_type_parameters.span_mut_or_empty(data_definition.type_parameters)
            {
                type_parameter.symbol =
                    next_child_of_kind(&mut data_children, symbols, SymbolKind::TypeParameter);
            }

            for member in data_members.span_mut_or_empty(data_definition.members) {
                match member {
                    omega_symbol_resolved_trees::data::DataMember::Field(field) => {
                        field.symbol =
                            next_child_of_kind(&mut data_children, symbols, SymbolKind::Field);
                    }
                    omega_symbol_resolved_trees::data::DataMember::Variant(variant) => {
                        variant.symbol =
                            next_child_of_kind(&mut data_children, symbols, SymbolKind::Variant);
                    }
                }
            }
        });

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
        machine.symbol = next_child_of_kind(&mut root_children, symbols, SymbolKind::Machine);
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

    let declarations = &mut program.tables.declarations;
    let data_type_parameters = &mut declarations.data_type_parameters;
    let state_parameters = &mut declarations.state_parameters;
    let child_type_references = &mut declarations.child_type_references;
    program.roots.operators.for_each_mut(|operator| {
        assign_operator_symbols(
            symbols,
            &mut root_children,
            data_type_parameters,
            state_parameters,
            child_type_references,
            operator,
        );
    });

    let declarations = &mut program.tables.declarations;
    let platform_state_signatures = &mut declarations.platform_state_signatures;
    let state_parameters = &mut declarations.state_parameters;
    let child_type_references = &mut declarations.child_type_references;
    program.roots.platforms.for_each_mut(|platform| {
        platform.symbol = next_child_of_kind(&mut root_children, symbols, SymbolKind::Platform);
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

    let declarations = &mut program.tables.declarations;
    let trait_requirements = &mut declarations.trait_requirements;
    let trait_machine_signatures = &mut declarations.trait_machine_signatures;
    let state_parameters = &mut declarations.state_parameters;
    let child_type_references = &mut declarations.child_type_references;
    program.roots.traits.for_each_mut(|trait_definition| {
        trait_definition.symbol =
            next_child_of_kind(&mut root_children, symbols, SymbolKind::Trait);
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

pub(super) fn inherited_field_count<'data>(
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

pub(super) fn assign_operator_symbols(
    symbols: &SymbolTable,
    siblings: &mut impl Iterator<Item = SymbolHandle>,
    data_type_parameters: &mut Arena<omega_symbol_resolved_trees::data::TypeParameter>,
    state_parameters: &mut Arena<omega_symbol_resolved_trees::signature::StateParameter>,
    child_type_references: &mut Arena<omega_symbol_resolved_trees::types::TypeReference>,
    operator: &mut omega_symbol_resolved_trees::operator::OperatorDefinition,
) {
    operator.symbol = next_child_of_kind(siblings, symbols, SymbolKind::Operator);
    let mut operator_children = symbols.child_handles(operator.symbol).into_iter().flatten();

    for type_parameter in data_type_parameters.span_mut_or_empty(operator.type_parameters) {
        type_parameter.symbol =
            next_child_of_kind(&mut operator_children, symbols, SymbolKind::TypeParameter);
    }
    let local_type_parameters = data_type_parameters
        .span_or_empty(operator.type_parameters)
        .to_vec();
    for parameter in state_parameters.span_mut_or_empty(operator.parameters) {
        parameter.symbol =
            next_child_of_kind(&mut operator_children, symbols, SymbolKind::Parameter);
        assign_type_reference_symbol_with_locals(
            symbols,
            child_type_references,
            &local_type_parameters,
            &mut parameter.type_reference,
        );
    }
    if let Some(return_type) = &mut operator.return_type {
        assign_type_reference_symbol_with_locals(
            symbols,
            child_type_references,
            &local_type_parameters,
            return_type,
        );
    }
}
