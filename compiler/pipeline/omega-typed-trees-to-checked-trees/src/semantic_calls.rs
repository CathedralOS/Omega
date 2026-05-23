use super::*;
use crate::lookup::{
    call_receiver_parts, machine_by_symbol, receiver_can_dispatch_to_machine,
    resolve_state_call_target, statement_call_can_dispatch_to_machine,
};

pub(crate) enum CallSite<'program> {
    Statement(&'program omega_typed_trees::statement::TableCall),
    Expression(&'program omega_typed_trees::expression::TableCallExpression),
}

pub(crate) fn find_call_site<'program>(
    program: &'program omega_typed_trees::TypedTrees,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    call_ordinal: usize,
) -> Option<CallSite<'program>> {
    let state = find_state_in_machine(program, machine_symbol, state_symbol)?;
    let machine = machine_by_symbol(program, machine_symbol)?;

    for (current_statement_index, statement) in program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .enumerate()
    {
        let mut current_ordinal = 0usize;
        if let Some(call_site) = find_call_site_in_statement(
            program,
            machine,
            state,
            statement,
            current_statement_index,
            statement_index,
            call_ordinal,
            &mut current_ordinal,
        ) {
            return Some(call_site);
        }
    }

    None
}

fn find_call_site_in_statement<'program>(
    program: &'program omega_typed_trees::TypedTrees,
    machine: &'program omega_typed_trees::machine::Machine,
    state: &'program omega_typed_trees::state::State,
    statement: &'program StatementNode,
    current_statement_index: usize,
    target_statement_index: usize,
    target_call_ordinal: usize,
    current_ordinal: &mut usize,
) -> Option<CallSite<'program>> {
    match statement {
        StatementNode::Assignment(assignment) => find_call_site_in_expression(
            program,
            machine,
            state,
            assignment.value,
            current_statement_index,
            target_statement_index,
            target_call_ordinal,
            current_ordinal,
        ),
        StatementNode::Call(call) => {
            let is_machine_call =
                statement_call_can_dispatch_to_machine(program, machine, state, call)
                    || call.target_symbol.is_valid();
            if is_machine_call {
                if current_statement_index == target_statement_index
                    && *current_ordinal == target_call_ordinal
                {
                    return Some(CallSite::Statement(call));
                }
                *current_ordinal = current_ordinal.saturating_add(1);
            }

            for argument in program.statement_table.expression_handles(call.arguments) {
                if let Some(call_site) = find_call_site_in_expression(
                    program,
                    machine,
                    state,
                    *argument,
                    current_statement_index,
                    target_statement_index,
                    target_call_ordinal,
                    current_ordinal,
                ) {
                    return Some(call_site);
                }
            }

            None
        }
        StatementNode::Expression(expression) => find_call_site_in_expression(
            program,
            machine,
            state,
            *expression,
            current_statement_index,
            target_statement_index,
            target_call_ordinal,
            current_ordinal,
        ),
        StatementNode::LocalData(local_data) => {
            if !local_data.initial_value.is_valid() {
                return None;
            }
            find_call_site_in_expression(
                program,
                machine,
                state,
                local_data.initial_value,
                current_statement_index,
                target_statement_index,
                target_call_ordinal,
                current_ordinal,
            )
        }
        StatementNode::Transition(transition) => {
            if let TransitionGuardNode::When(expression) = transition.guard
                && let Some(call_site) = find_call_site_in_expression(
                    program,
                    machine,
                    state,
                    expression,
                    current_statement_index,
                    target_statement_index,
                    target_call_ordinal,
                    current_ordinal,
                )
            {
                return Some(call_site);
            }

            if let Some(call_site) = find_call_site_in_transition_target(
                program,
                machine,
                state,
                transition.target,
                current_statement_index,
                target_statement_index,
                target_call_ordinal,
                current_ordinal,
            ) {
                return Some(call_site);
            }

            if transition.continuation.is_valid() {
                return find_call_site_in_transition_target(
                    program,
                    machine,
                    state,
                    transition.continuation,
                    current_statement_index,
                    target_statement_index,
                    target_call_ordinal,
                    current_ordinal,
                );
            }

            None
        }
    }
}

fn find_call_site_in_transition_target<'program>(
    program: &'program omega_typed_trees::TypedTrees,
    machine: &'program omega_typed_trees::machine::Machine,
    state: &'program omega_typed_trees::state::State,
    target: omega_typed_trees::statement::TransitionTargetHandle,
    current_statement_index: usize,
    target_statement_index: usize,
    target_call_ordinal: usize,
    current_ordinal: &mut usize,
) -> Option<CallSite<'program>> {
    match program.statement_table.transition_target(target) {
        TransitionTargetNode::Named { arguments, .. } => {
            for argument in program.statement_table.expression_handles(*arguments) {
                if let Some(call_site) = find_call_site_in_expression(
                    program,
                    machine,
                    state,
                    *argument,
                    current_statement_index,
                    target_statement_index,
                    target_call_ordinal,
                    current_ordinal,
                ) {
                    return Some(call_site);
                }
            }
            None
        }
        TransitionTargetNode::Value(expression) => find_call_site_in_expression(
            program,
            machine,
            state,
            *expression,
            current_statement_index,
            target_statement_index,
            target_call_ordinal,
            current_ordinal,
        ),
        TransitionTargetNode::SelfTarget | TransitionTargetNode::Terminal => None,
    }
}

fn find_call_site_in_expression<'program>(
    program: &'program omega_typed_trees::TypedTrees,
    machine: &'program omega_typed_trees::machine::Machine,
    state: &'program omega_typed_trees::state::State,
    expression: ExpressionHandle,
    current_statement_index: usize,
    target_statement_index: usize,
    target_call_ordinal: usize,
    current_ordinal: &mut usize,
) -> Option<CallSite<'program>> {
    match program.expression_table.expression(expression) {
        ExpressionNode::ArrayLiteral(values) => {
            for value in program.expression_table.expression_handles(*values) {
                if let Some(call_site) = find_call_site_in_expression(
                    program,
                    machine,
                    state,
                    *value,
                    current_statement_index,
                    target_statement_index,
                    target_call_ordinal,
                    current_ordinal,
                ) {
                    return Some(call_site);
                }
            }
            None
        }
        ExpressionNode::Binary(binary) => find_call_site_in_expression(
            program,
            machine,
            state,
            binary.left,
            current_statement_index,
            target_statement_index,
            target_call_ordinal,
            current_ordinal,
        )
        .or_else(|| {
            find_call_site_in_expression(
                program,
                machine,
                state,
                binary.right,
                current_statement_index,
                target_statement_index,
                target_call_ordinal,
                current_ordinal,
            )
        }),
        ExpressionNode::Call(call) => {
            let (receiver_symbol, receiver_path) = call_receiver_parts(program, call.receiver);
            let is_machine_call = resolve_state_call_target(
                program,
                machine,
                state,
                receiver_symbol,
                call.target_symbol,
                receiver_path.as_deref(),
                &call.target,
            )
            .is_valid()
                || receiver_can_dispatch_to_machine(
                    program,
                    machine,
                    state,
                    receiver_symbol,
                    receiver_path.as_deref(),
                )
                || call.target_symbol.is_valid();

            if is_machine_call {
                if current_statement_index == target_statement_index
                    && *current_ordinal == target_call_ordinal
                {
                    return Some(CallSite::Expression(call));
                }
                *current_ordinal = current_ordinal.saturating_add(1);
            }

            if call.receiver.is_valid()
                && let Some(call_site) = find_call_site_in_expression(
                    program,
                    machine,
                    state,
                    call.receiver,
                    current_statement_index,
                    target_statement_index,
                    target_call_ordinal,
                    current_ordinal,
                )
            {
                return Some(call_site);
            }

            for argument in program.expression_table.expression_handles(call.arguments) {
                if let Some(call_site) = find_call_site_in_expression(
                    program,
                    machine,
                    state,
                    *argument,
                    current_statement_index,
                    target_statement_index,
                    target_call_ordinal,
                    current_ordinal,
                ) {
                    return Some(call_site);
                }
            }

            None
        }
        ExpressionNode::Cast(cast) => find_call_site_in_expression(
            program,
            machine,
            state,
            cast.value,
            current_statement_index,
            target_statement_index,
            target_call_ordinal,
            current_ordinal,
        ),
        ExpressionNode::Indexed(indexed) => find_call_site_in_expression(
            program,
            machine,
            state,
            indexed.collection,
            current_statement_index,
            target_statement_index,
            target_call_ordinal,
            current_ordinal,
        )
        .or_else(|| {
            find_call_site_in_expression(
                program,
                machine,
                state,
                indexed.index,
                current_statement_index,
                target_statement_index,
                target_call_ordinal,
                current_ordinal,
            )
        }),
        ExpressionNode::Member(member) => find_call_site_in_expression(
            program,
            machine,
            state,
            member.receiver,
            current_statement_index,
            target_statement_index,
            target_call_ordinal,
            current_ordinal,
        ),
        ExpressionNode::Mutable(inner) => find_call_site_in_expression(
            program,
            machine,
            state,
            *inner,
            current_statement_index,
            target_statement_index,
            target_call_ordinal,
            current_ordinal,
        ),
        ExpressionNode::StructLiteral(struct_literal) => {
            for field in program.expression_table.struct_fields(struct_literal.fields) {
                if let Some(call_site) = find_call_site_in_expression(
                    program,
                    machine,
                    state,
                    field.value,
                    current_statement_index,
                    target_statement_index,
                    target_call_ordinal,
                    current_ordinal,
                ) {
                    return Some(call_site);
                }
            }
            None
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_) => None,
    }
}

pub(crate) fn call_site_argument_expressions<'program>(
    program: &'program omega_typed_trees::TypedTrees,
    call_site: &CallSite<'program>,
) -> &'program [ExpressionHandle] {
    match call_site {
        CallSite::Statement(call) => program.statement_table.expression_handles(call.arguments),
        CallSite::Expression(call) => program.expression_table.expression_handles(call.arguments),
    }
}

pub(crate) fn find_state_in_machine<'program>(
    program: &'program omega_typed_trees::TypedTrees,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
) -> Option<&'program omega_typed_trees::state::State> {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)?;
    program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == state_symbol)
}

pub(crate) fn find_state<'program>(
    program: &'program omega_typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
) -> Option<&'program omega_typed_trees::state::State> {
    program.machines().iter().find_map(|machine| {
        program
            .machine_states(machine)
            .iter()
            .find(|state| state.symbol == state_symbol)
    })
}
