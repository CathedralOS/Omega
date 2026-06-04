use super::*;

pub(in crate::flow::ownership) fn expression_type_reference_in_state(
    program: &omega_typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
) -> Option<omega_typed_trees::types::TypeReferenceHandle> {
    if !expression.is_valid() {
        return None;
    }

    match program.expression_table.expression(expression) {
        ExpressionNode::Mutable(inner) => {
            expression_type_reference_in_state(program, state_symbol, statement_index, *inner)
        }
        ExpressionNode::Name(_) | ExpressionNode::Member(_) | ExpressionNode::Indexed(_) => {
            let place = canonical_place_from_expression_in_state(
                program,
                state_symbol,
                statement_index,
                expression,
            )?;
            canonical_place_type_reference(program, state_symbol, statement_index, &place)
        }
        ExpressionNode::ArrayLiteral(_)
        | ExpressionNode::Binary(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::Call(_)
        | ExpressionNode::Cast(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Range(_)
        | ExpressionNode::String(_)
        | ExpressionNode::StructLiteral(_)
        | ExpressionNode::Unary(_) => None,
    }
}

fn canonical_place_type_reference(
    program: &omega_typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    place: &CanonicalPlace,
) -> Option<omega_typed_trees::types::TypeReferenceHandle> {
    let omega_facts::PlaceRoot::Symbol(root_symbol) = place.root else {
        return None;
    };

    let mut current =
        symbol_type_reference_in_state(program, state_symbol, statement_index, root_symbol)?;

    for segment in &place.segments {
        match segment {
            omega_facts::PlaceSegment::Field { symbol } => {
                current = field_type_reference(program, current, *symbol)?;
            }
            omega_facts::PlaceSegment::Index { .. } => {
                current = indexed_element_type_reference(program, current)?;
            }
        }
    }

    Some(current)
}

fn symbol_type_reference_in_state(
    program: &omega_typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    symbol: SymbolHandle,
) -> Option<omega_typed_trees::types::TypeReferenceHandle> {
    let state = find_state(program, state_symbol)?;

    program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.symbol == symbol)
        .map(|parameter| parameter.type_reference)
        .or_else(|| {
            program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .take(statement_index)
                .find_map(|statement| {
                    let StatementNode::LocalData(local_data) = statement else {
                        return None;
                    };
                    (local_data.symbol == symbol).then_some(local_data.type_reference)
                })
        })
        .or_else(|| machine_member_type_reference(program, state_symbol, symbol))
}

fn machine_member_type_reference(
    program: &omega_typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    symbol: SymbolHandle,
) -> Option<omega_typed_trees::types::TypeReferenceHandle> {
    program.machines().iter().find_map(|machine| {
        program
            .machine_states(machine)
            .iter()
            .any(|state| state.symbol == state_symbol)
            .then_some(machine)
            .and_then(|machine| {
                program
                    .machine_owned_data(machine)
                    .iter()
                    .find(|owned| owned.symbol == symbol)
                    .map(|owned| owned.type_reference)
                    .or_else(|| attached_data_field_type_reference(program, machine, symbol))
            })
    })
}

fn attached_data_field_type_reference(
    program: &omega_typed_trees::TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    symbol: SymbolHandle,
) -> Option<omega_typed_trees::types::TypeReferenceHandle> {
    let attached_data = machine.attached_data.as_deref()?;
    let data = program
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == attached_data)?;
    data_field_type_reference(program, data, symbol)
}

fn field_type_reference(
    program: &omega_typed_trees::TypedTrees,
    type_reference: omega_typed_trees::types::TypeReferenceHandle,
    field_symbol: SymbolHandle,
) -> Option<omega_typed_trees::types::TypeReferenceHandle> {
    match program.type_reference_table.type_reference(type_reference) {
        omega_typed_trees::types::TypeReferenceNode::Reference { referee, .. }
        | omega_typed_trees::types::TypeReferenceNode::Constrained {
            base_type: referee, ..
        } => field_type_reference(program, *referee, field_symbol),
        omega_typed_trees::types::TypeReferenceNode::Generic {
            base_symbol,
            base_name,
            ..
        }
        | omega_typed_trees::types::TypeReferenceNode::Named {
            symbol: base_symbol,
            name: base_name,
        } => data_definition_by_symbol_or_name(program, *base_symbol, base_name)
            .and_then(|data| data_field_type_reference(program, data, field_symbol)),
        omega_typed_trees::types::TypeReferenceNode::FixedArray { .. }
        | omega_typed_trees::types::TypeReferenceNode::Slice { .. }
        | omega_typed_trees::types::TypeReferenceNode::Unit => None,
    }
}

fn indexed_element_type_reference(
    program: &omega_typed_trees::TypedTrees,
    type_reference: omega_typed_trees::types::TypeReferenceHandle,
) -> Option<omega_typed_trees::types::TypeReferenceHandle> {
    match program.type_reference_table.type_reference(type_reference) {
        omega_typed_trees::types::TypeReferenceNode::Reference { referee, .. }
        | omega_typed_trees::types::TypeReferenceNode::Constrained {
            base_type: referee, ..
        } => indexed_element_type_reference(program, *referee),
        omega_typed_trees::types::TypeReferenceNode::FixedArray { element_type, .. }
        | omega_typed_trees::types::TypeReferenceNode::Slice { element_type } => {
            Some(*element_type)
        }
        omega_typed_trees::types::TypeReferenceNode::Generic { .. }
        | omega_typed_trees::types::TypeReferenceNode::Named { .. }
        | omega_typed_trees::types::TypeReferenceNode::Unit => None,
    }
}

fn data_definition_by_symbol_or_name<'program>(
    program: &'program omega_typed_trees::TypedTrees,
    symbol: SymbolHandle,
    name: &omega_typed_trees::name::Identifier,
) -> Option<&'program omega_typed_trees::data::DataDefinition> {
    program.data_definitions().iter().find(|definition| {
        (symbol.is_valid() && definition.symbol == symbol) || definition.name == *name
    })
}

fn data_field_type_reference(
    program: &omega_typed_trees::TypedTrees,
    data: &omega_typed_trees::data::DataDefinition,
    field_symbol: SymbolHandle,
) -> Option<omega_typed_trees::types::TypeReferenceHandle> {
    if !field_symbol.is_valid() {
        return None;
    }

    program.data_members(data).iter().find_map(|member| {
        let omega_typed_trees::data::DataMember::Field(field) = member else {
            return None;
        };
        (field.symbol == field_symbol).then_some(field.type_reference)
    })
}
