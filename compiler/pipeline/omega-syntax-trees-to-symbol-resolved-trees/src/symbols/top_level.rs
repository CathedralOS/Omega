mod machines;

use omega_core::arena::Arena;
use omega_core::symbols::{
    SymbolHandle, SymbolKind, SymbolTable, builtin_function_symbols, builtin_type_symbols,
};
use omega_symbol_resolved_trees::SymbolResolvedTrees;

use super::lookup::top_level_symbol;
use super::top_level::machines::assign_machine_symbols;
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

    assign_machine_symbols(program, symbols, &mut root_children);

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
