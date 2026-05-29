use super::*;

mod type_resolution;

use type_resolution::expression_requires_ownership;

pub(super) fn append_statement_ownership_events(
    program: &omega_typed_trees::TypedTrees,
    ctx: &mut FlowBuildContext,
    state_symbol: SymbolHandle,
    statement_index: usize,
    statement: &StatementNode,
) {
    match statement {
        StatementNode::Assignment(assignment) => append_move_events_for_expression(
            program,
            ctx,
            state_symbol,
            statement_index,
            assignment.value,
            FlowOwnershipEventSource::Statement { statement_index },
        ),
        StatementNode::LocalData(local_data) => {
            if type_requires_ownership(program, local_data.type_reference) {
                append_move_events_for_expression(
                    program,
                    ctx,
                    state_symbol,
                    statement_index,
                    local_data.initial_value,
                    FlowOwnershipEventSource::Statement { statement_index },
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
    machine: &omega_typed_trees::machine::Machine,
    state: &omega_typed_trees::state::State,
    borrow_call: &BorrowCallFact,
) {
    let Some(call_site) = find_call_site(
        program,
        machine.symbol,
        state.symbol,
        borrow_call.statement_index,
        borrow_call.call_ordinal,
    ) else {
        return;
    };
    let arguments = call_site_argument_expressions(program, &call_site);
    let Some(target_state) = find_state(program, borrow_call.target_symbol) else {
        return;
    };

    let parameters = program
        .state_parameters(target_state)
        .iter()
        .filter(|parameter| !parameter.is_self);

    for (parameter, argument) in parameters.zip(arguments.iter()) {
        if !type_requires_ownership(program, parameter.type_reference) {
            continue;
        }

        append_move_events_for_expression(
            program,
            ctx,
            state.symbol,
            borrow_call.statement_index,
            *argument,
            FlowOwnershipEventSource::Call {
                statement_index: borrow_call.statement_index,
                call_ordinal: borrow_call.call_ordinal,
                target_symbol: borrow_call.target_symbol,
            },
        );
    }
}

fn append_move_events_for_expression(
    program: &omega_typed_trees::TypedTrees,
    ctx: &mut FlowBuildContext,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
    source: FlowOwnershipEventSource,
) {
    if expression_requires_ownership(program, state_symbol, statement_index, expression) {
        if let Some(place) = canonical_place_from_expression_in_state(
            program,
            state_symbol,
            statement_index,
            expression,
        ) {
            append_move_event_for_place(ctx, place, source);
        }
        return;
    }

    match program.expression_table.expression(expression) {
        ExpressionNode::ArrayLiteral(values) => {
            for value in program.expression_table.expression_handles(*values) {
                append_move_events_for_expression(
                    program,
                    ctx,
                    state_symbol,
                    statement_index,
                    *value,
                    source,
                );
            }
        }
        ExpressionNode::Cast(cast) => append_move_events_for_expression(
            program,
            ctx,
            state_symbol,
            statement_index,
            cast.value,
            source,
        ),
        ExpressionNode::StructLiteral(struct_literal) => {
            for field in program
                .expression_table
                .struct_fields(struct_literal.fields)
            {
                append_move_events_for_expression(
                    program,
                    ctx,
                    state_symbol,
                    statement_index,
                    field.value,
                    source,
                );
            }
        }
        ExpressionNode::Mutable(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::Member(_)
        | ExpressionNode::Indexed(_)
        | ExpressionNode::Binary(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::Call(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Range(_)
        | ExpressionNode::String(_) => {}
    }
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
