//! Direct-call expression admission for stable and parameter-relative origins.

use super::local_aliases::expression_reborrows_stable_alias_binding;
use super::{
    ExpressionHandle, ExpressionNode, FramePlaceOrigin, Machine, MachineSymbols,
    ParameterRelativeFrameOrigin, StateParameter, SymbolHandle, TopLevelSymbols, TypedTrees,
    expression_is_effectful_for_transparent_result, expression_is_effectful_indexed_place,
    expression_reborrows_transparent_alias_binding, known_boundary_call_written_paths_for_parts,
    known_call_written_paths_for_parts, parameter_relative_place_origin, receiver_member_chain,
};

enum ExpressionAdmission {
    Reject,
    Leaf,
    Call,
}

enum PendingNode {
    Expression(ExpressionHandle),
    CallFrame(ExpressionHandle),
}

#[allow(clippy::too_many_arguments)]
pub(super) fn stable_alias_index_expression_preserves_origin(
    program: &TypedTrees,
    current_machine: &Machine,
    expression: ExpressionHandle,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    active_states: &mut Vec<SymbolHandle>,
    parameters: &[StateParameter],
    aliases: &[(String, FramePlaceOrigin)],
) -> bool {
    complete_direct_call_tree(
        program,
        current_machine,
        expression,
        machine_symbols,
        symbols,
        active_states,
        |expression, _| {
            if expression_reborrows_stable_alias_binding(program, expression, parameters, aliases) {
                ExpressionAdmission::Reject
            } else if !expression_is_effectful_for_transparent_result(program, expression) {
                ExpressionAdmission::Leaf
            } else {
                ExpressionAdmission::Call
            }
        },
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn statement_call_argument_preserves_transparent_result(
    program: &TypedTrees,
    current_machine: &Machine,
    expression: ExpressionHandle,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    active_states: &mut Vec<SymbolHandle>,
    parameters: &[StateParameter],
    aliases: &[(String, SymbolHandle, ParameterRelativeFrameOrigin)],
) -> bool {
    complete_direct_call_tree(
        program,
        current_machine,
        expression,
        machine_symbols,
        symbols,
        active_states,
        |expression, active_states| {
            if expression_reborrows_transparent_alias_binding(
                program, expression, parameters, aliases,
            ) {
                ExpressionAdmission::Reject
            } else if !expression_is_effectful_for_transparent_result(program, expression) {
                ExpressionAdmission::Leaf
            } else if expression_is_effectful_indexed_place(program, expression) {
                // Indexing needs its own origin proof at any call-tree position.
                // That proof checks the smaller index expressions and keeps the
                // first collection-coarse projection absorbing.
                if parameter_relative_place_origin(
                    program,
                    current_machine,
                    expression,
                    parameters,
                    aliases,
                    symbols,
                    active_states,
                )
                .is_some()
                {
                    ExpressionAdmission::Leaf
                } else {
                    ExpressionAdmission::Reject
                }
            } else {
                ExpressionAdmission::Call
            }
        },
    )
}

/// Walk the finite typed expression tree without a call-depth allowance. Each
/// context proves its leaves and rejects binding reborrows before the pure
/// shortcut. Every other node must be a direct call with a complete frame;
/// computed expression shells are not implicitly admitted here. Call-body
/// recursion remains guarded by the frame resolver's active-state checks.
fn complete_direct_call_tree(
    program: &TypedTrees,
    current_machine: &Machine,
    expression: ExpressionHandle,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    active_states: &mut Vec<SymbolHandle>,
    admit_expression: impl Fn(ExpressionHandle, &mut Vec<SymbolHandle>) -> ExpressionAdmission,
) -> bool {
    let mut pending = vec![PendingNode::Expression(expression)];
    while let Some(node) = pending.pop() {
        let expression = match node {
            PendingNode::Expression(expression) => {
                match admit_expression(expression, active_states) {
                    ExpressionAdmission::Reject => return false,
                    ExpressionAdmission::Leaf => continue,
                    ExpressionAdmission::Call => expression,
                }
            }
            PendingNode::CallFrame(expression) => expression,
        };
        let ExpressionNode::Call(call) = program.expression_table.expression(expression) else {
            return false;
        };
        let arguments = program.expression_table.expression_handles(call.arguments);
        if matches!(node, PendingNode::Expression(_)) {
            if call.receiver.is_valid()
                && expression_is_effectful_for_transparent_result(program, call.receiver)
            {
                return false;
            }
            // Check child expressions before the parent frame, in source order.
            pending.push(PendingNode::CallFrame(expression));
            pending.extend(arguments.iter().rev().copied().map(PendingNode::Expression));
            continue;
        }
        let receiver_members = if call.receiver.is_valid() {
            let Some(receiver) = receiver_member_chain(program, call.receiver) else {
                return false;
            };
            receiver
        } else {
            Vec::new()
        };
        if known_call_written_paths_for_parts(
            program,
            call.target_symbol,
            call.target.as_str(),
            &receiver_members,
            arguments,
            current_machine,
            machine_symbols,
            symbols,
            active_states,
        )
        .or_else(|| {
            known_boundary_call_written_paths_for_parts(
                program,
                machine_symbols,
                symbols,
                &receiver_members,
                call.target.as_str(),
                arguments,
            )
        })
        .is_none()
        {
            return false;
        }
    }
    true
}
