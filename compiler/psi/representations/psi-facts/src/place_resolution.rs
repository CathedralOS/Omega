use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode, TableMemberExpression};

use crate::{FactPlan, Place, PlaceHandle, PlaceRoot, PlaceSegment};

pub fn payload_variant_for_field(
    program: &TypedTrees,
    field_symbol: SymbolHandle,
) -> Option<SymbolHandle> {
    if !field_symbol.is_valid() {
        return None;
    }
    program.data_definitions().iter().find_map(|definition| {
        program.data_members(definition).iter().find_map(|member| {
            let psi_typed_trees::data::DataMember::Variant(variant) = member else {
                return None;
            };
            program
                .data_payload_fields(variant)
                .iter()
                .any(|field| field.symbol == field_symbol)
                .then_some(variant.symbol)
        })
    })
}

pub(crate) fn effective_member_symbol(
    program: &TypedTrees,
    receiver: ExpressionHandle,
    member: &TableMemberExpression,
) -> SymbolHandle {
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

fn resolve_member_symbol_from_receiver(
    program: &TypedTrees,
    receiver: ExpressionHandle,
    member_name: &str,
) -> Option<SymbolHandle> {
    let type_symbol = expression_type_symbol(program, receiver)?;

    if let Some(data) = program
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == type_symbol)
    {
        if let Some(symbol) = data_member_symbol_by_name(program, data, member_name) {
            return Some(symbol);
        }
    }

    if let Some(machine) = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == type_symbol)
    {
        if let Some(attached_data) = machine.attached_data.as_deref()
            && let Some(data) = program
                .data_definitions()
                .iter()
                .find(|definition| definition.name.as_str() == attached_data)
        {
            if let Some(symbol) = data_member_symbol_by_name(program, data, member_name) {
                return Some(symbol);
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

pub(crate) fn canonical_place_label(
    program: &TypedTrees,
    facts: &FactPlan,
    place: &Place,
) -> String {
    canonical_place_label_from_parts(
        program,
        place.root,
        facts.place_segments.span_or_empty(place.segments),
    )
}

fn canonical_place_label_from_parts(
    program: &TypedTrees,
    root: PlaceRoot,
    segments: &[PlaceSegment],
) -> String {
    let mut label = match root {
        PlaceRoot::Unknown => "unknown".to_owned(),
        PlaceRoot::Symbol(symbol) => symbol_label(program, symbol),
        PlaceRoot::Expression(expression) => program.expression_table.display_name(expression),
        PlaceRoot::TypeReference(type_reference) => program.display_type_reference(type_reference),
    };

    for segment in segments {
        match segment {
            PlaceSegment::Field { symbol } => {
                label.push('.');
                label.push_str(&symbol_label(program, *symbol));
            }
            PlaceSegment::Case { variant } => {
                label.push_str("::");
                label.push_str(&symbol_label(program, *variant));
            }
            PlaceSegment::FixedIndex { index } => {
                label.push('[');
                label.push_str(&index.to_string());
                label.push(']');
            }
            PlaceSegment::Index { expression } => {
                label.push('[');
                label.push_str(&program.expression_table.display_name(*expression));
                label.push(']');
            }
        }
    }

    label
}

fn symbol_label(program: &TypedTrees, symbol: SymbolHandle) -> String {
    for data in program.data_definitions() {
        if data.symbol == symbol {
            return data.name.as_str().to_owned();
        }

        for member in program.data_members(data) {
            match member {
                psi_typed_trees::data::DataMember::Field(field) if field.symbol == symbol => {
                    return field.name.as_str().to_owned();
                }
                psi_typed_trees::data::DataMember::Variant(variant) if variant.symbol == symbol => {
                    return variant.name.as_str().to_owned();
                }
                psi_typed_trees::data::DataMember::Variant(variant) => {
                    if let Some(field) = program
                        .data_payload_fields(variant)
                        .iter()
                        .find(|field| field.symbol == symbol)
                    {
                        return field.name.as_str().to_owned();
                    }
                }
                psi_typed_trees::data::DataMember::Field(_) => {}
            }
        }
    }

    for machine in program.machines() {
        if machine.symbol == symbol {
            return machine.name.as_str().to_owned();
        }
        for owned_data in program.machine_owned_data(machine) {
            if owned_data.symbol == symbol {
                return owned_data.name.as_str().to_owned();
            }
        }
        for state in program.machine_states(machine) {
            if state.symbol == symbol {
                return state.name.as_str().to_owned();
            }
            for parameter in program.state_parameters(state) {
                if parameter.symbol == symbol {
                    return parameter.name.as_str().to_owned();
                }
            }
        }
    }

    for trait_definition in program.traits() {
        if trait_definition.symbol == symbol {
            return trait_definition.name.as_str().to_owned();
        }
        for requirement in program.trait_requirements(trait_definition) {
            if requirement.symbol == symbol {
                return requirement.name.as_str().to_owned();
            }
        }
        for machine_signature in program.trait_machine_signatures(trait_definition) {
            if machine_signature.symbol == symbol {
                return machine_signature.name.as_str().to_owned();
            }
            for parameter in program.state_signature_parameters(machine_signature) {
                if parameter.symbol == symbol {
                    return parameter.name.as_str().to_owned();
                }
            }
        }
    }

    format!("symbol#{}", symbol.arena_index())
}

fn expression_type_symbol(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<SymbolHandle> {
    if !expression.is_valid() {
        return None;
    }

    match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(inner) => expression_type_symbol(program, inner.target),
        ExpressionNode::Name(path) => {
            let symbol = if path.head_symbol.is_valid() {
                path.head_symbol
            } else {
                path.symbol
            };
            symbol_type_symbol(program, symbol)
        }
        ExpressionNode::Member(member) => {
            let symbol = effective_member_symbol(program, member.receiver, member);
            symbol_type_symbol(program, symbol)
        }
        _ => None,
    }
}

fn symbol_type_symbol(program: &TypedTrees, symbol: SymbolHandle) -> Option<SymbolHandle> {
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
                    return Some(type_reference_base_symbol(
                        program,
                        parameter.type_reference,
                    ));
                }
            }
        }
        for owned in program.machine_owned_data(machine) {
            if owned.symbol == symbol {
                return Some(type_reference_base_symbol(program, owned.type_reference));
            }
        }
    }

    for data in program.data_definitions() {
        for member in program.data_members(data) {
            match member {
                psi_typed_trees::data::DataMember::Field(field) if field.symbol == symbol => {
                    return Some(type_reference_base_symbol(program, field.type_reference));
                }
                psi_typed_trees::data::DataMember::Variant(variant) => {
                    if let Some(field) = program
                        .data_payload_fields(variant)
                        .iter()
                        .find(|field| field.symbol == symbol)
                    {
                        return Some(type_reference_base_symbol(program, field.type_reference));
                    }
                }
                _ => {}
            }
        }
    }

    None
}

fn type_reference_base_symbol(
    program: &TypedTrees,
    type_reference: psi_typed_trees::types::TypeReferenceHandle,
) -> SymbolHandle {
    match program.type_reference_table.type_reference(type_reference) {
        psi_typed_trees::types::TypeReferenceNode::Reference { referee, .. } => {
            type_reference_base_symbol(program, *referee)
        }
        psi_typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
            type_reference_base_symbol(program, *base_type)
        }
        psi_typed_trees::types::TypeReferenceNode::Generic { base_symbol, .. }
        | psi_typed_trees::types::TypeReferenceNode::DynamicTrait {
            symbol: base_symbol,
            ..
        }
        | psi_typed_trees::types::TypeReferenceNode::Named {
            symbol: base_symbol,
            ..
        } => *base_symbol,
        psi_typed_trees::types::TypeReferenceNode::FixedArray { .. }
        | psi_typed_trees::types::TypeReferenceNode::Slice { .. }
        | psi_typed_trees::types::TypeReferenceNode::ConstExpression(_)
        | psi_typed_trees::types::TypeReferenceNode::Unit => SymbolHandle::invalid(),
    }
}

pub(crate) fn resolve_place_member_symbol(
    program: &TypedTrees,
    facts: &FactPlan,
    place: PlaceHandle,
    member_name: &str,
) -> Option<SymbolHandle> {
    let place = facts.places.get(place);
    let base_symbol = fact_place_type_symbol(program, facts, place)?;

    if let Some(machine) = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == base_symbol)
        && let Some(attached_data) = machine.attached_data.as_deref()
        && let Some(data) = program
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == attached_data)
    {
        if let Some(symbol) = data_member_symbol_by_name(program, data, member_name) {
            return Some(symbol);
        }
    }

    if let Some(data) = program
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == base_symbol)
    {
        if let Some(symbol) = data_member_symbol_by_name(program, data, member_name) {
            return Some(symbol);
        }
    }

    None
}

fn data_member_symbol_by_name(
    program: &TypedTrees,
    data: &psi_typed_trees::data::DataDefinition,
    member_name: &str,
) -> Option<SymbolHandle> {
    program
        .data_members(data)
        .iter()
        .find_map(|member| match member {
            psi_typed_trees::data::DataMember::Field(field) => {
                (field.name.as_str() == member_name).then_some(field.symbol)
            }
            psi_typed_trees::data::DataMember::Variant(variant) => (variant.name.as_str()
                == member_name)
                .then_some(variant.symbol)
                .or_else(|| {
                    program
                        .data_payload_fields(variant)
                        .iter()
                        .find_map(|field| {
                            (field.name.as_str() == member_name).then_some(field.symbol)
                        })
                }),
        })
}

fn fact_place_type_symbol(
    program: &TypedTrees,
    facts: &FactPlan,
    place: &Place,
) -> Option<SymbolHandle> {
    let mut current = match place.root {
        PlaceRoot::Symbol(symbol) => symbol_type_symbol(program, symbol)?,
        PlaceRoot::Expression(expression) => expression_type_symbol(program, expression)?,
        PlaceRoot::Unknown | PlaceRoot::TypeReference(_) => return None,
    };

    for segment in facts.place_segments.span_or_empty(place.segments) {
        match segment {
            PlaceSegment::Field { symbol } => {
                current = symbol_type_symbol(program, *symbol)?;
            }
            PlaceSegment::Case { .. } => {}
            PlaceSegment::FixedIndex { .. } | PlaceSegment::Index { .. } => return None,
        }
    }

    Some(current)
}
