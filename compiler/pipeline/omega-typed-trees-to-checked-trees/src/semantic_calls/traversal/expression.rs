use super::*;
use crate::lookup::{
    call_receiver_parts, receiver_can_dispatch_to_machine, resolve_state_call_target,
};

pub(super) fn find_call_site_in_expression<'program>(
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
        ExpressionNode::Range(range) => {
            if range.start.is_valid()
                && let Some(call_site) = find_call_site_in_expression(
                    program,
                    machine,
                    state,
                    range.start,
                    current_statement_index,
                    target_statement_index,
                    target_call_ordinal,
                    current_ordinal,
                )
            {
                return Some(call_site);
            }
            range.end.is_valid().then(|| {
                find_call_site_in_expression(
                    program,
                    machine,
                    state,
                    range.end,
                    current_statement_index,
                    target_statement_index,
                    target_call_ordinal,
                    current_ordinal,
                )
            })?
        }
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
            for field in program
                .expression_table
                .struct_fields(struct_literal.fields)
            {
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
