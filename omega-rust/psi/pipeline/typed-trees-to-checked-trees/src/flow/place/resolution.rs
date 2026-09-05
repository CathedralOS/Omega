use super::*;
use crate::lookup::{first_valid_name_path_symbol, machine_by_symbol};

pub(crate) fn effective_member_symbol(
    program: &typed_trees::TypedTrees,
    receiver: ExpressionHandle,
    member: &typed_trees::expression::TableMemberExpression,
) -> SymbolHandle {
    if let ExpressionNode::StructLiteral(literal) = program.expression_table.expression(receiver)
        && let Some(field) = program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .find(|field| field.name == member.member)
        && field.field_symbol.is_valid()
    {
        return field.field_symbol;
    }

    if let Some(symbol) =
        resolve_member_symbol_from_receiver(program, receiver, member.member.as_str())
    {
        return symbol;
    }

    if member.member_symbol.is_valid() {
        return member.member_symbol;
    }

    SymbolHandle::invalid()
}

pub(crate) fn resolve_member_symbol_from_type_symbol(
    program: &typed_trees::TypedTrees,
    type_symbol: SymbolHandle,
    member_name: &str,
) -> Option<SymbolHandle> {
    if let Some(data) = program
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == type_symbol)
    {
        for member in program.data_members(data) {
            match member {
                typed_trees::data::DataMember::Field(field)
                    if field.name.as_str() == member_name =>
                {
                    return Some(field.symbol);
                }
                typed_trees::data::DataMember::Variant(variant)
                    if variant.name.as_str() == member_name =>
                {
                    return Some(variant.symbol);
                }
                typed_trees::data::DataMember::Variant(variant) => {
                    if let Some(field) = program
                        .data_payload_fields(variant)
                        .iter()
                        .find(|field| field.name.as_str() == member_name)
                    {
                        return Some(field.symbol);
                    }
                }
                _ => {}
            }
        }
    }

    if let Some(machine) = machine_by_symbol(program, type_symbol) {
        if let Some(attached_data) = machine.attached_data.as_deref()
            && let Some(data) = program
                .data_definitions()
                .iter()
                .find(|definition| definition.name.as_str() == attached_data)
        {
            for member in program.data_members(data) {
                match member {
                    typed_trees::data::DataMember::Field(field)
                        if field.name.as_str() == member_name =>
                    {
                        return Some(field.symbol);
                    }
                    typed_trees::data::DataMember::Variant(variant)
                        if variant.name.as_str() == member_name =>
                    {
                        return Some(variant.symbol);
                    }
                    typed_trees::data::DataMember::Variant(variant) => {
                        if let Some(field) = program
                            .data_payload_fields(variant)
                            .iter()
                            .find(|field| field.name.as_str() == member_name)
                        {
                            return Some(field.symbol);
                        }
                    }
                    _ => {}
                }
            }
        }
        for owned in program.machine_owned_data(machine) {
            if owned.name.as_str() == member_name {
                return Some(owned.symbol);
            }
        }
    }

    None
}

fn resolve_member_symbol_from_receiver(
    program: &typed_trees::TypedTrees,
    receiver: ExpressionHandle,
    member_name: &str,
) -> Option<SymbolHandle> {
    let type_symbol = expression_type_symbol(program, receiver)?;
    resolve_member_symbol_from_type_symbol(program, type_symbol, member_name)
}

pub(crate) fn expression_type_symbol(
    program: &typed_trees::TypedTrees,
    expression: ExpressionHandle,
) -> Option<SymbolHandle> {
    if !expression.is_valid() {
        return None;
    }

    match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(inner) => expression_type_symbol(program, inner.target),
        ExpressionNode::Name(path) => {
            let symbol = first_valid_name_path_symbol(path, &program.expression_table)?;
            symbol_type_symbol(program, symbol)
        }
        ExpressionNode::Indexed(indexed) => expression_type_symbol(program, indexed.collection),
        ExpressionNode::Member(member) => {
            let symbol = effective_member_symbol(program, member.receiver, member);
            symbol_type_symbol(program, symbol)
        }
        ExpressionNode::StructLiteral(literal) => literal
            .type_symbol
            .is_valid()
            .then_some(literal.type_symbol),
        _ => None,
    }
}

pub(crate) fn symbol_type_symbol(
    program: &typed_trees::TypedTrees,
    symbol: SymbolHandle,
) -> Option<SymbolHandle> {
    if !symbol.is_valid() {
        return None;
    }

    for machine in program.machines() {
        if machine.symbol == symbol
            && let Some(attached_data) = machine.attached_data.as_deref()
            && let Some(data) = program
                .data_definitions()
                .iter()
                .find(|definition| definition.name.as_str() == attached_data)
        {
            return Some(data.symbol);
        }
        for state in program.machine_states(machine) {
            for parameter in program.state_parameters(state) {
                if parameter.symbol == symbol {
                    return Some(
                        program
                            .type_reference_table
                            .type_symbol(parameter.type_reference),
                    );
                }
            }
            for statement in program.statement_table.statements(state.statement_nodes) {
                if let typed_trees::statement::StatementNode::LocalData(local_data) = statement
                    && local_data.symbol == symbol
                {
                    return Some(
                        program
                            .type_reference_table
                            .type_symbol(local_data.type_reference),
                    );
                }
            }
        }
        for owned in program.machine_owned_data(machine) {
            if owned.symbol == symbol {
                return Some(
                    program
                        .type_reference_table
                        .type_symbol(owned.type_reference),
                );
            }
        }
    }

    // Trait requirements are checked state signatures rather than executable
    // machine states. Their parameter symbols still own exact declared types
    // and must participate in member-place resolution for requirement
    // contracts.
    for definition in program.traits() {
        for signature in program.trait_machine_signatures(definition) {
            for parameter in program.state_signature_parameters(signature) {
                if parameter.symbol == symbol {
                    return Some(
                        program
                            .type_reference_table
                            .type_symbol(parameter.type_reference),
                    );
                }
            }
        }
    }

    // Operator contracts are declaration-owned proof expressions rather than
    // executable state bodies. Their parameters nevertheless have ordinary
    // typed member-place semantics and must resolve through their exact
    // operator declaration.
    for operator in program.operators().iter().chain(
        program
            .domain_definitions()
            .iter()
            .flat_map(|domain| program.domain_operators(domain)),
    ) {
        for parameter in program.operator_parameters(operator) {
            if parameter.symbol == symbol {
                return Some(
                    program
                        .type_reference_table
                        .type_symbol(parameter.type_reference),
                );
            }
        }
    }

    for data in program.data_definitions() {
        for member in program.data_members(data) {
            match member {
                typed_trees::data::DataMember::Field(field) if field.symbol == symbol => {
                    return Some(
                        program
                            .type_reference_table
                            .type_symbol(field.type_reference),
                    );
                }
                typed_trees::data::DataMember::Variant(variant) => {
                    if let Some(field) = program
                        .data_payload_fields(variant)
                        .iter()
                        .find(|field| field.symbol == symbol)
                    {
                        return Some(
                            program
                                .type_reference_table
                                .type_symbol(field.type_reference),
                        );
                    }
                }
                _ => {}
            }
        }
    }

    None
}
