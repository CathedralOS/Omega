use super::*;

pub(super) fn append_statement_ownership_events(
    program: &omega_typed_trees::TypedTrees,
    ctx: &mut FlowBuildContext,
    state_symbol: SymbolHandle,
    statement_index: usize,
    statement: &StatementNode,
) {
    match statement {
        StatementNode::Assignment(assignment) => append_move_event_for_expression(
            program,
            ctx,
            state_symbol,
            statement_index,
            assignment.value,
        ),
        StatementNode::LocalData(local_data) => {
            if type_requires_ownership(program, local_data.type_reference) {
                append_move_event_for_expression(
                    program,
                    ctx,
                    state_symbol,
                    statement_index,
                    local_data.initial_value,
                );
            }
        }
        StatementNode::Call(_) | StatementNode::Expression(_) | StatementNode::Transition(_) => {}
    }
}

pub(super) fn append_state_exit_drop_events(
    program: &omega_typed_trees::TypedTrees,
    ctx: &mut FlowBuildContext,
    state: &omega_typed_trees::state::State,
) {
    for statement in program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .rev()
    {
        let StatementNode::LocalData(local_data) = statement else {
            continue;
        };

        if !type_requires_ownership(program, local_data.type_reference) {
            continue;
        }

        append_drop_event_for_place(
            ctx,
            CanonicalPlace {
                root: omega_facts::PlaceRoot::Symbol(local_data.symbol),
                segments: Vec::new(),
            },
            FlowOwnershipEventSource::StateExit,
        );
    }
}

pub(super) fn append_call_ownership_events(
    program: &omega_typed_trees::TypedTrees,
    ctx: &mut FlowBuildContext,
    state: &omega_typed_trees::state::State,
    borrow_call: &BorrowCallFact,
) {
    let Some(arguments) = statement_call_arguments(program, state, borrow_call) else {
        return;
    };
    let Some(target_state) = find_state(program, borrow_call.target_symbol) else {
        return;
    };

    let parameters = program
        .state_parameters(target_state)
        .iter()
        .filter(|parameter| !parameter.is_self);

    for (parameter, argument) in parameters.zip(arguments.iter()) {
        if !type_requires_ownership(program, parameter.type_reference)
            || !expression_is_place_like(program, *argument)
        {
            continue;
        }

        if let Some(place) = canonical_place_from_expression_in_state(
            program,
            state.symbol,
            borrow_call.statement_index,
            *argument,
        ) {
            append_move_event_for_place(
                ctx,
                place,
                FlowOwnershipEventSource::Call {
                    statement_index: borrow_call.statement_index,
                    call_ordinal: borrow_call.call_ordinal,
                    target_symbol: borrow_call.target_symbol,
                },
            );
        }
    }
}

fn append_move_event_for_expression(
    program: &omega_typed_trees::TypedTrees,
    ctx: &mut FlowBuildContext,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
) {
    if !expression_requires_ownership(program, state_symbol, statement_index, expression) {
        return;
    }

    if let Some(place) =
        canonical_place_from_expression_in_state(program, state_symbol, statement_index, expression)
    {
        append_move_event_for_place(
            ctx,
            place,
            FlowOwnershipEventSource::Statement { statement_index },
        );
    }
}

fn expression_requires_ownership(
    program: &omega_typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
) -> bool {
    if !expression_is_place_like(program, expression) {
        return false;
    }

    expression_type_reference_in_state(program, state_symbol, statement_index, expression)
        .map(|type_reference| type_requires_ownership(program, type_reference))
        .unwrap_or(true)
}

fn expression_is_place_like(
    program: &omega_typed_trees::TypedTrees,
    expression: ExpressionHandle,
) -> bool {
    if !expression.is_valid() {
        return false;
    }

    match program.expression_table.expression(expression) {
        ExpressionNode::Mutable(inner) => expression_is_place_like(program, *inner),
        ExpressionNode::Name(_) | ExpressionNode::Member(_) | ExpressionNode::Indexed(_) => true,
        ExpressionNode::ArrayLiteral(_)
        | ExpressionNode::Binary(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::Call(_)
        | ExpressionNode::Cast(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Range(_)
        | ExpressionNode::String(_)
        | ExpressionNode::StructLiteral(_) => false,
    }
}

fn statement_call_arguments<'a>(
    program: &'a omega_typed_trees::TypedTrees,
    state: &omega_typed_trees::state::State,
    borrow_call: &BorrowCallFact,
) -> Option<&'a [ExpressionHandle]> {
    let statement = program
        .statement_table
        .statements(state.statement_nodes)
        .get(borrow_call.statement_index)?;
    let StatementNode::Call(call) = statement else {
        return None;
    };
    (call.target_symbol == borrow_call.target_symbol
        && call.receiver_symbol == borrow_call.receiver_symbol)
        .then(|| program.statement_table.expression_handles(call.arguments))
}

fn expression_type_reference_in_state(
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
        | ExpressionNode::StructLiteral(_) => None,
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

fn type_requires_ownership(
    program: &omega_typed_trees::TypedTrees,
    type_reference: omega_typed_trees::types::TypeReferenceHandle,
) -> bool {
    if !type_reference.is_valid() {
        return false;
    }

    match program.type_reference_table.type_reference(type_reference) {
        omega_typed_trees::types::TypeReferenceNode::Reference { .. } => false,
        omega_typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
            type_requires_ownership(program, *base_type)
        }
        omega_typed_trees::types::TypeReferenceNode::FixedArray { element_type, .. } => {
            type_requires_ownership(program, *element_type)
        }
        omega_typed_trees::types::TypeReferenceNode::Named { name, .. } => !matches!(
            omega_typed_trees::types::PrimitiveType::from_name(name.as_str()),
            Some(
                omega_typed_trees::types::PrimitiveType::Bool
                    | omega_typed_trees::types::PrimitiveType::F32
                    | omega_typed_trees::types::PrimitiveType::F64
                    | omega_typed_trees::types::PrimitiveType::I32
                    | omega_typed_trees::types::PrimitiveType::U32
                    | omega_typed_trees::types::PrimitiveType::U64
                    | omega_typed_trees::types::PrimitiveType::Usize
            )
        ),
        omega_typed_trees::types::TypeReferenceNode::Slice { .. }
        | omega_typed_trees::types::TypeReferenceNode::Generic { .. } => true,
        omega_typed_trees::types::TypeReferenceNode::Unit => false,
    }
}

fn append_move_event_for_place(
    ctx: &mut FlowBuildContext,
    place: CanonicalPlace,
    source: FlowOwnershipEventSource,
) {
    ctx.moves.append(FlowMoveEventFact {
        source,
        root: place.root,
        segments: ctx.ownership_segments.insert_many(place.segments),
    });
}

fn append_drop_event_for_place(
    ctx: &mut FlowBuildContext,
    place: CanonicalPlace,
    source: FlowOwnershipEventSource,
) {
    ctx.drops.append(FlowDropEventFact {
        source,
        root: place.root,
        segments: ctx.ownership_segments.insert_many(place.segments),
    });
}
