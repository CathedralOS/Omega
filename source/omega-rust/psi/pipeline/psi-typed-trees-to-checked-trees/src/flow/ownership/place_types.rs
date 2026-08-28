use super::*;

pub(crate) fn expression_type_reference_in_state(
    program: &psi_typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
) -> Option<psi_typed_trees::types::TypeReferenceHandle> {
    if !expression.is_valid() {
        return None;
    }

    match program.expression_table.expression(expression) {
        ExpressionNode::Atomic(atomic) => {
            expression_type_reference_in_state(program, state_symbol, statement_index, atomic.value)
        }
        ExpressionNode::Borrow(inner) => {
            expression_type_reference_in_state(program, state_symbol, statement_index, inner.target)
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
        | ExpressionNode::Unary(_)
        | ExpressionNode::ZeroValue(_) => None,
    }
}

pub(crate) fn canonical_place_type_reference(
    program: &psi_typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    place: &CanonicalPlace,
) -> Option<psi_typed_trees::types::TypeReferenceHandle> {
    let psi_facts::PlaceRoot::Symbol(root_symbol) = place.root else {
        return None;
    };

    // `self` move events are normalized to the durable machine symbol before
    // checked facts are published. Recover projections through that machine's
    // attached data shape so ownership validation can still inspect every
    // nominal prefix.
    if let Some(machine) = program.machines().iter().find(|machine| {
        machine.symbol == root_symbol
            && program
                .machine_states(machine)
                .iter()
                .any(|state| state.symbol == state_symbol)
    }) && let Some((psi_facts::PlaceSegment::Field { symbol }, remaining)) =
        place.segments.split_first()
    {
        let current = attached_data_field_type_reference(program, machine, *symbol)?;
        return project_type_reference_from_segments(program, current, remaining);
    }

    let current =
        symbol_type_reference_in_state(program, state_symbol, statement_index, root_symbol)?;
    project_type_reference_from_segments(program, current, &place.segments)
}

pub(crate) fn project_type_reference_from_segments(
    program: &psi_typed_trees::TypedTrees,
    mut current: psi_typed_trees::types::TypeReferenceHandle,
    segments: &[psi_facts::PlaceSegment],
) -> Option<psi_typed_trees::types::TypeReferenceHandle> {
    let mut substitutions = Vec::new();
    for segment in segments {
        current = substituted_type_reference(program, current, &substitutions);
        match segment {
            psi_facts::PlaceSegment::Case { .. } => {}
            psi_facts::PlaceSegment::Field { symbol } => {
                current = field_type_reference(program, current, *symbol, &mut substitutions)?;
            }
            psi_facts::PlaceSegment::FixedIndex { .. } | psi_facts::PlaceSegment::Index { .. } => {
                current = indexed_element_type_reference(program, current, &substitutions)?;
            }
            psi_facts::PlaceSegment::FixedRange { .. } => return None,
        }
    }

    Some(substituted_type_reference(program, current, &substitutions))
}

fn symbol_type_reference_in_state(
    program: &psi_typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    symbol: SymbolHandle,
) -> Option<psi_typed_trees::types::TypeReferenceHandle> {
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
    program: &psi_typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    symbol: SymbolHandle,
) -> Option<psi_typed_trees::types::TypeReferenceHandle> {
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
    program: &psi_typed_trees::TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    symbol: SymbolHandle,
) -> Option<psi_typed_trees::types::TypeReferenceHandle> {
    let attached_data = machine.attached_data.as_deref()?;
    let data = program
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == attached_data)?;
    data_field_type_reference(program, data, symbol)
}

fn field_type_reference(
    program: &psi_typed_trees::TypedTrees,
    type_reference: psi_typed_trees::types::TypeReferenceHandle,
    field_symbol: SymbolHandle,
    substitutions: &mut Vec<(SymbolHandle, psi_typed_trees::types::TypeReferenceHandle)>,
) -> Option<psi_typed_trees::types::TypeReferenceHandle> {
    let type_reference = substituted_type_reference(program, type_reference, substitutions);
    match program.type_reference_table.type_reference(type_reference) {
        psi_typed_trees::types::TypeReferenceNode::Reference { referee, .. }
        | psi_typed_trees::types::TypeReferenceNode::Constrained {
            base_type: referee, ..
        } => field_type_reference(program, *referee, field_symbol, substitutions),
        psi_typed_trees::types::TypeReferenceNode::Generic {
            base_symbol,
            base_name,
            arguments,
            ..
        } => {
            let data = data_definition_by_symbol_or_name(program, *base_symbol, base_name)?;
            let bindings: Vec<_> = program
                .data_type_parameters(data)
                .iter()
                .zip(
                    program
                        .type_reference_table
                        .type_reference_handles(*arguments),
                )
                .filter_map(|(parameter, argument)| {
                    matches!(
                        parameter.kind,
                        psi_typed_trees::data::TypeParameterKind::Type
                    )
                    .then(|| {
                        (
                            parameter.symbol,
                            substituted_type_reference(program, *argument, substitutions),
                        )
                    })
                })
                .collect();
            substitutions.extend(bindings);
            data_field_type_reference(program, data, field_symbol)
        }
        psi_typed_trees::types::TypeReferenceNode::Named {
            symbol: base_symbol,
            name: base_name,
        } => data_definition_by_symbol_or_name(program, *base_symbol, base_name)
            .and_then(|data| data_field_type_reference(program, data, field_symbol)),
        psi_typed_trees::types::TypeReferenceNode::ConstExpression(_)
        | psi_typed_trees::types::TypeReferenceNode::FixedArray { .. }
        | psi_typed_trees::types::TypeReferenceNode::DynamicTrait { .. }
        | psi_typed_trees::types::TypeReferenceNode::Slice { .. }
        | psi_typed_trees::types::TypeReferenceNode::Unit => None,
    }
}

fn indexed_element_type_reference(
    program: &psi_typed_trees::TypedTrees,
    type_reference: psi_typed_trees::types::TypeReferenceHandle,
    substitutions: &[(SymbolHandle, psi_typed_trees::types::TypeReferenceHandle)],
) -> Option<psi_typed_trees::types::TypeReferenceHandle> {
    let type_reference = substituted_type_reference(program, type_reference, substitutions);
    match program.type_reference_table.type_reference(type_reference) {
        psi_typed_trees::types::TypeReferenceNode::Reference { referee, .. }
        | psi_typed_trees::types::TypeReferenceNode::Constrained {
            base_type: referee, ..
        } => indexed_element_type_reference(program, *referee, substitutions),
        psi_typed_trees::types::TypeReferenceNode::FixedArray { element_type, .. }
        | psi_typed_trees::types::TypeReferenceNode::Slice { element_type } => Some(*element_type),
        psi_typed_trees::types::TypeReferenceNode::ConstExpression(_)
        | psi_typed_trees::types::TypeReferenceNode::Generic { .. }
        | psi_typed_trees::types::TypeReferenceNode::Named { .. }
        | psi_typed_trees::types::TypeReferenceNode::DynamicTrait { .. }
        | psi_typed_trees::types::TypeReferenceNode::Unit => None,
    }
}

fn substituted_type_reference(
    program: &psi_typed_trees::TypedTrees,
    mut type_reference: psi_typed_trees::types::TypeReferenceHandle,
    substitutions: &[(SymbolHandle, psi_typed_trees::types::TypeReferenceHandle)],
) -> psi_typed_trees::types::TypeReferenceHandle {
    let mut remaining = substitutions.len().saturating_add(1);
    while remaining > 0 {
        remaining -= 1;
        let psi_typed_trees::types::TypeReferenceNode::Named { symbol, .. } =
            program.type_reference_table.type_reference(type_reference)
        else {
            break;
        };
        let Some(replacement) = substitutions
            .iter()
            .rev()
            .find_map(|(parameter, replacement)| (*parameter == *symbol).then_some(*replacement))
        else {
            break;
        };
        if replacement == type_reference {
            break;
        }
        type_reference = replacement;
    }
    type_reference
}

fn data_definition_by_symbol_or_name<'program>(
    program: &'program psi_typed_trees::TypedTrees,
    symbol: SymbolHandle,
    name: &psi_typed_trees::name::Identifier,
) -> Option<&'program psi_typed_trees::data::DataDefinition> {
    program.data_definitions().iter().find(|definition| {
        (symbol.is_valid() && definition.symbol == symbol) || definition.name == *name
    })
}

fn data_field_type_reference(
    program: &psi_typed_trees::TypedTrees,
    data: &psi_typed_trees::data::DataDefinition,
    field_symbol: SymbolHandle,
) -> Option<psi_typed_trees::types::TypeReferenceHandle> {
    if !field_symbol.is_valid() {
        return None;
    }

    program
        .data_members(data)
        .iter()
        .find_map(|member| match member {
            psi_typed_trees::data::DataMember::Field(field) => {
                (field.symbol == field_symbol).then_some(field.type_reference)
            }
            psi_typed_trees::data::DataMember::Variant(variant) => program
                .data_payload_fields(variant)
                .iter()
                .find_map(|field| (field.symbol == field_symbol).then_some(field.type_reference)),
        })
}
