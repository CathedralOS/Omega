use crate::context::*;
mod accesses;
mod calls;
mod roots;

use crate::lookup::{first_valid_name_path_symbol, machine_state_count};
use crate::semantic_calls::find_state;
use accesses::borrow_access_place;
use calls::collect_statement_borrow_calls;
use roots::{append_state_writable_roots, estimated_borrow_root_capacity, mutable_parameter_count};

#[derive(Clone)]
struct StateLoanTracker {
    handle: Handle<omega_checked_trees::BorrowLoanFact>,
    owner_symbol: SymbolHandle,
    owner_name: Identifier,
    place: accesses::BorrowAccessPlace,
}

pub(crate) fn build_borrow_facts(program: &omega_typed_trees::TypedTrees) -> BorrowFacts {
    let mut writable_roots =
        omega_core::arena::Arena::with_capacity(estimated_borrow_root_capacity(program));
    let mut access_segments =
        omega_core::arena::Arena::with_capacity(program.expression_table.expression_count());
    let mut argument_accesses =
        omega_core::arena::Arena::with_capacity(program.expression_table.expression_count());
    let mut calls =
        omega_core::arena::Arena::with_capacity(program.statement_table.statement_count());
    let mut loans =
        omega_core::arena::Arena::with_capacity(program.statement_table.statement_count());
    let mut states = omega_core::arena::Arena::with_capacity(machine_state_count(program));
    let mut state_loan_trackers = Vec::new();

    for machine in program.machines() {
        for state in program.machine_states(machine) {
            state_loan_trackers.clear();
            let mut writable_roots_span = omega_core::arena::HandleSpan::empty();
            append_state_writable_roots(
                program,
                machine,
                state,
                &mut writable_roots,
                &mut writable_roots_span,
            );

            let mut calls_span = omega_core::arena::HandleSpan::empty();
            let mut loans_span = omega_core::arena::HandleSpan::empty();
            for (statement_index, statement) in program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .enumerate()
            {
                if let Some((owner_symbol, owner_name, place, source_owner_symbol, kind)) =
                    statement_borrow_loan(
                        program,
                        state,
                        statement_index,
                        machine.symbol,
                        statement,
                        &state_loan_trackers,
                    )
                {
                    let loan_segments = access_segments.insert_many(place.segments.clone());
                    let handle = loans.append_to_span(
                        &mut loans_span,
                        omega_checked_trees::BorrowLoanFact {
                            statement_index,
                            last_use_statement_index: statement_index,
                            owner_symbol,
                            source_owner_symbol,
                            root_symbol: place.root_symbol,
                            segments: loan_segments,
                            kind,
                        },
                    );
                    state_loan_trackers.push(StateLoanTracker {
                        handle,
                        owner_symbol,
                        owner_name,
                        place,
                    });
                }
                let mut call_ordinal = 0usize;
                collect_statement_borrow_calls(
                    program,
                    machine,
                    state,
                    statement_index,
                    statement,
                    &mut call_ordinal,
                    &mut access_segments,
                    &mut argument_accesses,
                    &mut calls,
                    &mut calls_span,
                );
            }

            update_state_loan_last_uses(
                program,
                state.statement_nodes,
                calls.span_or_empty(calls_span),
                &argument_accesses,
                &state_loan_trackers,
                &mut loans,
            );

            states.append(StateBorrowFact {
                machine_symbol: machine.symbol,
                state_symbol: state.symbol,
                writable_roots: writable_roots_span,
                mutable_parameter_count: mutable_parameter_count(program, state),
                calls: calls_span,
                loans: loans_span,
            });
        }
    }

    BorrowFacts {
        writable_roots,
        access_segments,
        argument_accesses,
        calls,
        loans,
        states,
    }
}

fn statement_borrow_loan(
    program: &omega_typed_trees::TypedTrees,
    state: &omega_typed_trees::state::State,
    statement_index: usize,
    machine_symbol: SymbolHandle,
    statement: &StatementNode,
    loan_trackers: &[StateLoanTracker],
) -> Option<(
    SymbolHandle,
    Identifier,
    accesses::BorrowAccessPlace,
    SymbolHandle,
    omega_checked_trees::BorrowAccessKind,
)> {
    match statement {
        StatementNode::LocalData(local_data) => {
            if !is_reference_type(program, local_data.type_reference) {
                return None;
            }

            let local_is_mutable_reference =
                is_mutable_reference_type(program, local_data.type_reference);
            let place = match program
                .expression_table
                .expression(local_data.initial_value)
            {
                omega_checked_trees::expression::ExpressionNode::Mutable(inner_expression)
                    if local_is_mutable_reference =>
                {
                    borrow_access_place(
                        program,
                        state.symbol,
                        statement_index,
                        *inner_expression,
                        machine_symbol,
                    )
                }
                omega_checked_trees::expression::ExpressionNode::Call(call) => {
                    helper_call_borrow_loan_place(
                        program,
                        state.symbol,
                        statement_index,
                        machine_symbol,
                        call,
                    )
                }
                omega_checked_trees::expression::ExpressionNode::Indexed(_) => borrow_access_place(
                    program,
                    state.symbol,
                    statement_index,
                    local_data.initial_value,
                    machine_symbol,
                ),
                _ => None,
            }?;

            let (place, source_owner_symbol) =
                rebase_borrow_place_through_local_loan(place, loan_trackers);

            Some((
                local_data.symbol,
                local_data.name.clone(),
                place,
                source_owner_symbol,
                if local_is_mutable_reference {
                    omega_checked_trees::BorrowAccessKind::Mutable
                } else {
                    omega_checked_trees::BorrowAccessKind::Read
                },
            ))
        }
        StatementNode::Assignment(_)
        | StatementNode::Call(_)
        | StatementNode::Expression(_)
        | StatementNode::Transition(_) => None,
    }
}

fn helper_call_borrow_loan_place(
    program: &omega_typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    machine_symbol: SymbolHandle,
    call: &omega_checked_trees::expression::TableCallExpression,
) -> Option<accesses::BorrowAccessPlace> {
    if !call.receiver.is_valid() {
        return None;
    }

    if matches!(
        call.target.as_str(),
        "as_slice" | "as_mut_slice" | "as_view"
    ) {
        return borrow_access_place(
            program,
            state_symbol,
            statement_index,
            call.receiver,
            machine_symbol,
        );
    }

    let Some(target_state) = find_state(program, call.target_symbol) else {
        return None;
    };
    let receiver_is_self = program
        .state_parameters(target_state)
        .iter()
        .any(|parameter| parameter.is_self);

    if !receiver_is_self || !is_reference_type(program, target_state.return_type) {
        return None;
    }

    borrow_access_place(
        program,
        state_symbol,
        statement_index,
        call.receiver,
        machine_symbol,
    )
}

fn rebase_borrow_place_through_local_loan(
    place: accesses::BorrowAccessPlace,
    loan_trackers: &[StateLoanTracker],
) -> (accesses::BorrowAccessPlace, SymbolHandle) {
    let Some(source_loan) = loan_trackers
        .iter()
        .rev()
        .find(|loan| loan.owner_symbol == place.root_symbol)
    else {
        return (place, SymbolHandle::invalid());
    };

    let mut rebased_segments = Vec::with_capacity(
        source_loan
            .place
            .segments
            .len()
            .saturating_add(place.segments.len()),
    );
    rebased_segments.extend(source_loan.place.segments.iter().copied());
    rebased_segments.extend(place.segments.iter().copied());

    (
        accesses::BorrowAccessPlace {
            root_symbol: source_loan.place.root_symbol,
            segments: rebased_segments,
        },
        source_loan.owner_symbol,
    )
}

fn is_reference_type(
    program: &omega_typed_trees::TypedTrees,
    type_reference: omega_typed_trees::types::TypeReferenceHandle,
) -> bool {
    match program.type_reference_table.type_reference(type_reference) {
        omega_typed_trees::types::TypeReferenceNode::Reference { .. } => true,
        omega_typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
            is_reference_type(program, *base_type)
        }
        omega_typed_trees::types::TypeReferenceNode::FixedArray { .. }
        | omega_typed_trees::types::TypeReferenceNode::Generic { .. }
        | omega_typed_trees::types::TypeReferenceNode::Named { .. }
        | omega_typed_trees::types::TypeReferenceNode::Slice { .. }
        | omega_typed_trees::types::TypeReferenceNode::Unit => false,
    }
}

fn is_mutable_reference_type(
    program: &omega_typed_trees::TypedTrees,
    type_reference: omega_typed_trees::types::TypeReferenceHandle,
) -> bool {
    match program.type_reference_table.type_reference(type_reference) {
        omega_typed_trees::types::TypeReferenceNode::Reference { is_mutable, .. } => *is_mutable,
        omega_typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
            is_mutable_reference_type(program, *base_type)
        }
        omega_typed_trees::types::TypeReferenceNode::FixedArray { .. }
        | omega_typed_trees::types::TypeReferenceNode::Generic { .. }
        | omega_typed_trees::types::TypeReferenceNode::Named { .. }
        | omega_typed_trees::types::TypeReferenceNode::Slice { .. }
        | omega_typed_trees::types::TypeReferenceNode::Unit => false,
    }
}

fn update_state_loan_last_uses(
    program: &omega_typed_trees::TypedTrees,
    statements: omega_core::arena::HandleSpan<StatementNode>,
    borrow_calls: &[BorrowCallFact],
    argument_accesses: &omega_core::arena::Arena<BorrowArgumentAccessFact>,
    loan_trackers: &[StateLoanTracker],
    loans: &mut omega_core::arena::Arena<omega_checked_trees::BorrowLoanFact>,
) {
    if loan_trackers.is_empty() {
        return;
    }

    for borrow_call in borrow_calls {
        for access in argument_accesses.span_or_empty(borrow_call.accesses) {
            for tracker in loan_trackers {
                if tracker.owner_symbol == access.root_symbol {
                    loans.get_mut(tracker.handle).last_use_statement_index =
                        borrow_call.statement_index;
                }
            }
        }
    }

    for (statement_index, statement) in program
        .statement_table
        .statements(statements)
        .iter()
        .enumerate()
    {
        for tracker in loan_trackers {
            if statement_uses_local_name(program, statement, tracker.owner_name.as_str())
                || statement_uses_symbol(program, statement, tracker.owner_symbol)
            {
                loans.get_mut(tracker.handle).last_use_statement_index = statement_index;
            }
        }
    }
}

fn statement_uses_local_name(
    program: &omega_typed_trees::TypedTrees,
    statement: &StatementNode,
    local_name: &str,
) -> bool {
    match statement {
        StatementNode::Assignment(assignment) => {
            expression_uses_local_name(program, assignment.target, local_name)
                || expression_uses_local_name(program, assignment.value, local_name)
        }
        StatementNode::Call(call) => {
            program
                .statement_table
                .name_path_members(call.receiver)
                .first()
                .is_some_and(|member| member.as_str() == local_name)
                || program
                    .statement_table
                    .expression_handles(call.arguments)
                    .iter()
                    .any(|argument| expression_uses_local_name(program, *argument, local_name))
        }
        StatementNode::Expression(expression) => {
            expression_uses_local_name(program, *expression, local_name)
        }
        StatementNode::LocalData(local_data) => {
            expression_uses_local_name(program, local_data.initial_value, local_name)
        }
        StatementNode::Transition(transition) => {
            transition_guard_uses_local_name(program, transition.guard, local_name)
                || transition_target_uses_local_name(
                    program,
                    program.statement_table.transition_target(transition.target),
                    local_name,
                )
                || transition_target_uses_local_name(
                    program,
                    program
                        .statement_table
                        .transition_target(transition.continuation),
                    local_name,
                )
        }
    }
}

fn statement_uses_symbol(
    program: &omega_typed_trees::TypedTrees,
    statement: &StatementNode,
    symbol: SymbolHandle,
) -> bool {
    match statement {
        StatementNode::Assignment(assignment) => {
            expression_uses_symbol(program, assignment.target, symbol)
                || expression_uses_symbol(program, assignment.value, symbol)
        }
        StatementNode::Call(call) => {
            call.receiver_symbol == symbol
                || program
                    .statement_table
                    .expression_handles(call.arguments)
                    .iter()
                    .any(|argument| expression_uses_symbol(program, *argument, symbol))
        }
        StatementNode::Expression(expression) => {
            expression_uses_symbol(program, *expression, symbol)
        }
        StatementNode::LocalData(local_data) => {
            expression_uses_symbol(program, local_data.initial_value, symbol)
        }
        StatementNode::Transition(transition) => {
            transition_guard_uses_symbol(program, transition.guard, symbol)
                || transition_target_uses_symbol(
                    program,
                    program.statement_table.transition_target(transition.target),
                    symbol,
                )
                || transition_target_uses_symbol(
                    program,
                    program
                        .statement_table
                        .transition_target(transition.continuation),
                    symbol,
                )
        }
    }
}

fn transition_guard_uses_symbol(
    program: &omega_typed_trees::TypedTrees,
    guard: omega_typed_trees::statement::TransitionGuardNode,
    symbol: SymbolHandle,
) -> bool {
    match guard {
        omega_typed_trees::statement::TransitionGuardNode::Always => false,
        omega_typed_trees::statement::TransitionGuardNode::When(expression) => {
            expression_uses_symbol(program, expression, symbol)
        }
    }
}

fn transition_guard_uses_local_name(
    program: &omega_typed_trees::TypedTrees,
    guard: omega_typed_trees::statement::TransitionGuardNode,
    local_name: &str,
) -> bool {
    match guard {
        omega_typed_trees::statement::TransitionGuardNode::Always => false,
        omega_typed_trees::statement::TransitionGuardNode::When(expression) => {
            expression_uses_local_name(program, expression, local_name)
        }
    }
}

fn transition_target_uses_symbol(
    program: &omega_typed_trees::TypedTrees,
    target: &omega_typed_trees::statement::TransitionTargetNode,
    symbol: SymbolHandle,
) -> bool {
    match target {
        omega_typed_trees::statement::TransitionTargetNode::Named { path, arguments } => {
            path.head_symbol == symbol
                || path.symbol == symbol
                || program
                    .statement_table
                    .expression_handles(*arguments)
                    .iter()
                    .any(|argument| expression_uses_symbol(program, *argument, symbol))
        }
        omega_typed_trees::statement::TransitionTargetNode::Value(expression) => {
            expression_uses_symbol(program, *expression, symbol)
        }
        omega_typed_trees::statement::TransitionTargetNode::SelfTarget
        | omega_typed_trees::statement::TransitionTargetNode::Terminal => false,
    }
}

fn transition_target_uses_local_name(
    program: &omega_typed_trees::TypedTrees,
    target: &omega_typed_trees::statement::TransitionTargetNode,
    local_name: &str,
) -> bool {
    match target {
        omega_typed_trees::statement::TransitionTargetNode::Named { path, arguments } => {
            program
                .statement_table
                .name_path_members(path.members)
                .first()
                .is_some_and(|member| member.as_str() == local_name)
                || program
                    .statement_table
                    .expression_handles(*arguments)
                    .iter()
                    .any(|argument| expression_uses_local_name(program, *argument, local_name))
        }
        omega_typed_trees::statement::TransitionTargetNode::Value(expression) => {
            expression_uses_local_name(program, *expression, local_name)
        }
        omega_typed_trees::statement::TransitionTargetNode::SelfTarget
        | omega_typed_trees::statement::TransitionTargetNode::Terminal => false,
    }
}

fn expression_uses_symbol(
    program: &omega_typed_trees::TypedTrees,
    expression: ExpressionHandle,
    symbol: SymbolHandle,
) -> bool {
    match program.expression_table.expression(expression) {
        omega_typed_trees::expression::ExpressionNode::ArrayLiteral(values) => program
            .expression_table
            .expression_handles(*values)
            .iter()
            .any(|value| expression_uses_symbol(program, *value, symbol)),
        omega_typed_trees::expression::ExpressionNode::Binary(binary) => {
            expression_uses_symbol(program, binary.left, symbol)
                || expression_uses_symbol(program, binary.right, symbol)
        }
        omega_typed_trees::expression::ExpressionNode::Call(call) => {
            (call.receiver.is_valid() && expression_uses_symbol(program, call.receiver, symbol))
                || program
                    .expression_table
                    .expression_handles(call.arguments)
                    .iter()
                    .any(|argument| expression_uses_symbol(program, *argument, symbol))
        }
        omega_typed_trees::expression::ExpressionNode::Cast(cast) => {
            expression_uses_symbol(program, cast.value, symbol)
        }
        omega_typed_trees::expression::ExpressionNode::Indexed(indexed) => {
            expression_uses_symbol(program, indexed.collection, symbol)
                || expression_uses_symbol(program, indexed.index, symbol)
        }
        omega_typed_trees::expression::ExpressionNode::Range(range) => {
            (range.start.is_valid() && expression_uses_symbol(program, range.start, symbol))
                || (range.end.is_valid() && expression_uses_symbol(program, range.end, symbol))
        }
        omega_typed_trees::expression::ExpressionNode::Member(member) => {
            expression_uses_symbol(program, member.receiver, symbol)
        }
        omega_typed_trees::expression::ExpressionNode::Mutable(inner_expression) => {
            expression_uses_symbol(program, *inner_expression, symbol)
        }
        omega_typed_trees::expression::ExpressionNode::Name(path) => {
            first_valid_name_path_symbol(path, &program.expression_table)
                .is_some_and(|path_symbol| path_symbol == symbol)
                || path.symbol == symbol
        }
        omega_typed_trees::expression::ExpressionNode::StructLiteral(struct_literal) => program
            .expression_table
            .struct_fields(struct_literal.fields)
            .iter()
            .any(|field| expression_uses_symbol(program, field.value, symbol)),
        omega_typed_trees::expression::ExpressionNode::Boolean(_)
        | omega_typed_trees::expression::ExpressionNode::Float(_)
        | omega_typed_trees::expression::ExpressionNode::Integer(_)
        | omega_typed_trees::expression::ExpressionNode::String(_) => false,
    }
}

fn expression_uses_local_name(
    program: &omega_typed_trees::TypedTrees,
    expression: ExpressionHandle,
    local_name: &str,
) -> bool {
    match program.expression_table.expression(expression) {
        omega_typed_trees::expression::ExpressionNode::ArrayLiteral(values) => program
            .expression_table
            .expression_handles(*values)
            .iter()
            .any(|value| expression_uses_local_name(program, *value, local_name)),
        omega_typed_trees::expression::ExpressionNode::Binary(binary) => {
            expression_uses_local_name(program, binary.left, local_name)
                || expression_uses_local_name(program, binary.right, local_name)
        }
        omega_typed_trees::expression::ExpressionNode::Call(call) => {
            (call.receiver.is_valid()
                && expression_uses_local_name(program, call.receiver, local_name))
                || program
                    .expression_table
                    .expression_handles(call.arguments)
                    .iter()
                    .any(|argument| expression_uses_local_name(program, *argument, local_name))
        }
        omega_typed_trees::expression::ExpressionNode::Cast(cast) => {
            expression_uses_local_name(program, cast.value, local_name)
        }
        omega_typed_trees::expression::ExpressionNode::Indexed(indexed) => {
            expression_uses_local_name(program, indexed.collection, local_name)
                || expression_uses_local_name(program, indexed.index, local_name)
        }
        omega_typed_trees::expression::ExpressionNode::Range(range) => {
            (range.start.is_valid() && expression_uses_local_name(program, range.start, local_name))
                || (range.end.is_valid()
                    && expression_uses_local_name(program, range.end, local_name))
        }
        omega_typed_trees::expression::ExpressionNode::Member(member) => {
            expression_uses_local_name(program, member.receiver, local_name)
        }
        omega_typed_trees::expression::ExpressionNode::Mutable(inner_expression) => {
            expression_uses_local_name(program, *inner_expression, local_name)
        }
        omega_typed_trees::expression::ExpressionNode::Name(path) => program
            .expression_table
            .name_path_members(path.members)
            .first()
            .is_some_and(|member| member.as_str() == local_name),
        omega_typed_trees::expression::ExpressionNode::StructLiteral(struct_literal) => program
            .expression_table
            .struct_fields(struct_literal.fields)
            .iter()
            .any(|field| expression_uses_local_name(program, field.value, local_name)),
        omega_typed_trees::expression::ExpressionNode::Boolean(_)
        | omega_typed_trees::expression::ExpressionNode::Float(_)
        | omega_typed_trees::expression::ExpressionNode::Integer(_)
        | omega_typed_trees::expression::ExpressionNode::String(_) => false,
    }
}
