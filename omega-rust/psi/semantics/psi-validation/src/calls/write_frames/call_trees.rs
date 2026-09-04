//! Worklist expression admission for stable and parameter-relative origins.

use super::call_targets::call_argument_types;
use super::local_aliases::expression_reborrows_stable_alias_binding;
use super::value_expressions::{ValuePosition, push_value_children, value_call_result_is_admitted};
use super::{
    ExpressionHandle, ExpressionNode, FramePlaceOrigin, Machine, MachineSymbols,
    ParameterRelativeFrameOrigin, StateParameter, SymbolHandle, TopLevelSymbols,
    TypeReferenceHandle, TypedTrees, expression_is_effectful_for_transparent_result,
    expression_is_effectful_indexed_place, expression_reborrows_transparent_alias_binding,
    known_boundary_call_written_paths_for_parts, known_call_written_paths_for_parts,
    parameter_relative_place_origin, receiver_member_chain,
};

enum ExpressionAdmission {
    Reject,
    Leaf,
    Call,
    Value(ValuePosition),
}

enum PendingNode {
    Expression(ExpressionHandle, Option<ValuePosition>),
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
    complete_expression_tree(
        program,
        current_machine,
        expression,
        None,
        machine_symbols,
        symbols,
        active_states,
        |expression, _, _| {
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
    expected_type: TypeReferenceHandle,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    active_states: &mut Vec<SymbolHandle>,
    parameters: &[StateParameter],
    aliases: &[(String, SymbolHandle, ParameterRelativeFrameOrigin)],
) -> bool {
    complete_expression_tree(
        program,
        current_machine,
        expression,
        Some(ValuePosition::CallArgument(expected_type)),
        machine_symbols,
        symbols,
        active_states,
        |expression, position, active_states| {
            if expression_reborrows_transparent_alias_binding(
                program, expression, parameters, aliases,
            ) {
                ExpressionAdmission::Reject
            } else if !expression_is_effectful_for_transparent_result(program, expression) {
                ExpressionAdmission::Leaf
            } else if matches!(position, Some(ValuePosition::CallArgument(_)))
                && expression_is_effectful_indexed_place(program, expression)
            {
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
                } else if matches!(
                    program.expression_table.expression(expression),
                    ExpressionNode::Borrow(_)
                ) {
                    ExpressionAdmission::Reject
                } else {
                    // A scalar projection of a computed literal need not have
                    // a place origin. Its typed value path checks every child;
                    // borrowed places cannot use that fallback.
                    ExpressionAdmission::Value(position.expect("argument position"))
                }
            } else if let Some(position) = position {
                ExpressionAdmission::Value(position)
            } else {
                ExpressionAdmission::Call
            }
        },
    )
}

/// Walk the finite typed expression tree without a call-depth allowance. Each
/// context proves its leaves and rejects binding reborrows before the pure
/// shortcut. Parameter-relative arguments additionally expand typed value
/// structure using the assignment/initializer rules. Stable indexes keep the
/// direct-call policy. Call-body recursion remains guarded by the frame
/// resolver's active-state checks.
#[allow(clippy::too_many_arguments)]
fn complete_expression_tree(
    program: &TypedTrees,
    current_machine: &Machine,
    expression: ExpressionHandle,
    position: Option<ValuePosition>,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    active_states: &mut Vec<SymbolHandle>,
    admit_expression: impl Fn(
        ExpressionHandle,
        Option<ValuePosition>,
        &mut Vec<SymbolHandle>,
    ) -> ExpressionAdmission,
) -> bool {
    let mut pending = vec![PendingNode::Expression(expression, position)];
    let mut value_children = Vec::new();
    while let Some(node) = pending.pop() {
        let expression = match node {
            PendingNode::Expression(expression, position) => {
                match admit_expression(expression, position, active_states) {
                    ExpressionAdmission::Reject => return false,
                    ExpressionAdmission::Leaf => continue,
                    ExpressionAdmission::Call => expression,
                    ExpressionAdmission::Value(position) => {
                        if matches!(
                            program.expression_table.expression(expression),
                            ExpressionNode::Call(_)
                        ) {
                            if !value_call_result_is_admitted(
                                program, expression, position, symbols,
                            ) {
                                return false;
                            }
                            expression
                        } else {
                            value_children.clear();
                            if !push_value_children(
                                program,
                                expression,
                                position,
                                &mut value_children,
                            ) {
                                return false;
                            }
                            pending.extend(value_children.drain(..).map(|(child, position)| {
                                PendingNode::Expression(child, Some(position))
                            }));
                            continue;
                        }
                    }
                }
            }
            PendingNode::CallFrame(expression) => expression,
        };
        let ExpressionNode::Call(call) = program.expression_table.expression(expression) else {
            return false;
        };
        let arguments = program.expression_table.expression_handles(call.arguments);
        if matches!(node, PendingNode::Expression(..)) {
            if call.receiver.is_valid()
                && expression_is_effectful_for_transparent_result(program, call.receiver)
            {
                return false;
            }
            // Check child expressions before the parent frame, in source order.
            pending.push(PendingNode::CallFrame(expression));
            let argument_types = if position.is_some() {
                call_argument_types(
                    program,
                    call.target_symbol,
                    call.target.as_str(),
                    !call.receiver.is_valid(),
                    symbols,
                )
            } else {
                Vec::new()
            };
            pending.extend(arguments.iter().enumerate().rev().map(|(index, argument)| {
                let expected_type = argument_types.get(index).copied().unwrap_or_default();
                PendingNode::Expression(
                    *argument,
                    position.map(|_| ValuePosition::CallArgument(expected_type)),
                )
            }));
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
