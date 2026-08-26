use crate::InstructionSelectionInput;
use omega_control_flow::StateKey;
use omega_runtime_bodies::{RuntimeDispatchBodyOperation, RuntimeDispatchBodyOperationKind};
use omega_state_calls::StateCallRole;
use psi_checked_trees::expression::{
    ExpressionHandle, ExpressionNode, ExpressionTable, TableBinaryExpression, TableBorrowExpression,
};
use psi_checked_trees::statement::StatementNode;
use psi_checked_trees::types::{PrimitiveType, TypeReferenceHandle};
use psi_numerics::literals::{IntegerLanding, LandedIntegerType};
use psi_symbols::SymbolHandle;

use super::super::bindings::{
    RuntimeAliasBinding, RuntimeAliasBuffer, resolve_runtime_alias_binding_handle,
    strip_mutable_expression_handle,
};
use super::super::lookups::{
    state_assignment_value_call, state_assignment_value_call_by_ordinal,
    state_call_argument_call_by_ordinal, state_call_for_statement, state_transition_argument_call,
    state_transition_argument_call_by_ordinal, state_transition_guard_call,
};

pub(super) fn bind_runtime_operation_aliases(
    input: &InstructionSelectionInput<'_>,
    operation: &RuntimeDispatchBodyOperation,
    aliases: &mut RuntimeAliasBuffer,
    alias_expressions: &mut ExpressionTable,
) {
    bind_prior_local_aliases(input, operation, aliases, alias_expressions);

    match &operation.kind {
        RuntimeDispatchBodyOperationKind::InlineLeafStateCall { .. }
        | RuntimeDispatchBodyOperationKind::InlineStateCall { .. }
        | RuntimeDispatchBodyOperationKind::StateCall { .. } => {}
        RuntimeDispatchBodyOperationKind::DynamicStateCall { .. }
        | RuntimeDispatchBodyOperationKind::HostCall { .. }
        | RuntimeDispatchBodyOperationKind::MachineHalt
        | RuntimeDispatchBodyOperationKind::MemoryFence(_)
        | RuntimeDispatchBodyOperationKind::InterruptControl(_)
        | RuntimeDispatchBodyOperationKind::FlagsSnapshot
        | RuntimeDispatchBodyOperationKind::FlagsRestore
        | RuntimeDispatchBodyOperationKind::MsrRead
        | RuntimeDispatchBodyOperationKind::MsrWrite
        | RuntimeDispatchBodyOperationKind::ControlRegisterRead(_)
        | RuntimeDispatchBodyOperationKind::ControlRegisterWrite(_)
        | RuntimeDispatchBodyOperationKind::PortWrite
        | RuntimeDispatchBodyOperationKind::PortRead
        | RuntimeDispatchBodyOperationKind::LocalStorage { .. }
        | RuntimeDispatchBodyOperationKind::Mutation { .. }
        | RuntimeDispatchBodyOperationKind::StateCallResult { .. }
        | RuntimeDispatchBodyOperationKind::Other => return,
    }

    let (role, call_ordinal) = match &operation.kind {
        RuntimeDispatchBodyOperationKind::InlineLeafStateCall {
            role, call_ordinal, ..
        }
        | RuntimeDispatchBodyOperationKind::InlineStateCall {
            role, call_ordinal, ..
        }
        | RuntimeDispatchBodyOperationKind::StateCall {
            role, call_ordinal, ..
        } => (*role, *call_ordinal),
        _ => unreachable!(),
    };

    let state_call = match role {
        StateCallRole::Statement => {
            state_call_for_statement(input, operation.source_key, operation.statement_index)
        }
        StateCallRole::AssignmentValue => state_assignment_value_call_by_ordinal(
            input,
            operation.source_key,
            operation.statement_index,
            call_ordinal,
        )
        .or_else(|| {
            state_assignment_value_call(input, operation.source_key, operation.statement_index)
        }),
        StateCallRole::CallArgument => state_call_argument_call_by_ordinal(
            input,
            operation.source_key,
            operation.statement_index,
            call_ordinal,
        ),
        StateCallRole::TransitionGuard => {
            state_transition_guard_call(input, operation.source_key, operation.statement_index)
        }
        StateCallRole::TransitionArgument => state_transition_argument_call_by_ordinal(
            input,
            operation.source_key,
            operation.statement_index,
            call_ordinal,
        )
        .or_else(|| {
            state_transition_argument_call(input, operation.source_key, operation.statement_index)
        }),
    };
    let Some(state_call) = state_call else {
        return;
    };
    let Some(arguments) = input.state_calls.arguments.span(state_call.arguments) else {
        return;
    };

    for argument in arguments {
        let argument_expression =
            alias_expressions.copy_from(&input.state_calls.expressions, argument.expression);
        let resolved_expression = resolve_runtime_alias_binding_handle(
            argument_expression,
            state_call.source_key,
            aliases.bindings(),
            alias_expressions,
        );
        let expression =
            strip_mutable_expression_handle(alias_expressions, resolved_expression.expression);
        let expression = state_parameter_integer_landing(
            input,
            state_call.target_key,
            argument.parameter_symbol,
        )
        .map(|landing| {
            stamp_anonymous_integer_landing_on_value_spine(alias_expressions, expression, landing)
        })
        .unwrap_or(expression);
        aliases.set_alias(RuntimeAliasBinding {
            source_key: state_call.target_key,
            parameter_symbol: argument.parameter_symbol,
            parameter_name: argument.parameter_name.clone(),
            expression_source_key: resolved_expression.source_key,
            expression,
        });
    }
}

fn bind_prior_local_aliases(
    input: &InstructionSelectionInput<'_>,
    operation: &RuntimeDispatchBodyOperation,
    aliases: &mut RuntimeAliasBuffer,
    alias_expressions: &mut ExpressionTable,
) {
    let Some(machine) = input
        .program
        .machines()
        .iter()
        .find(|machine| machine.symbol == operation.source_key.machine)
    else {
        return;
    };
    let Some(state) = input
        .program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == operation.source_key.state)
    else {
        return;
    };

    let statements = input
        .program
        .statement_table
        .statements(state.statement_nodes);
    for (statement_index, statement) in statements
        .iter()
        .enumerate()
        .take(operation.statement_index)
    {
        let StatementNode::LocalData(local_data) = statement else {
            continue;
        };
        if !local_data.initial_value.is_valid()
            || local_initial_value_is_call(input, local_data.initial_value)
            || local_requires_runtime_storage(
                input,
                operation.source_key,
                statement_index,
                local_data.symbol,
            )
            || local_is_assigned_between(
                input,
                statements,
                statement_index + 1,
                operation.statement_index,
                local_data.symbol,
            )
        {
            // A local whose initializer is a CALL has its result materialized once
            // into its own call-result slot; do NOT alias it to the call expression
            // (that would re-reference the call at a later statement where it is not
            // collected, breaking value/write lowering -- esp. in a dispatched callee).
            // The local stays a Name and resolves to its call-result slot.
            continue;
        }

        let initializer =
            alias_expressions.copy_from(&input.program.expression_table, local_data.initial_value);
        let resolved_initializer = resolve_runtime_alias_binding_handle(
            initializer,
            operation.source_key,
            aliases.bindings(),
            alias_expressions,
        );
        let expression = integer_landing_for_type_reference(input, local_data.type_reference)
            .map(|landing| {
                stamp_anonymous_integer_landing_on_value_spine(
                    alias_expressions,
                    resolved_initializer.expression,
                    landing,
                )
            })
            .unwrap_or(resolved_initializer.expression);
        aliases.set_alias(RuntimeAliasBinding {
            source_key: operation.source_key,
            parameter_symbol: local_data.symbol,
            parameter_name: local_data.name.clone(),
            expression_source_key: resolved_initializer.source_key,
            expression,
        });
    }
}

fn state_parameter_integer_landing(
    input: &InstructionSelectionInput<'_>,
    source_key: StateKey,
    parameter_symbol: SymbolHandle,
) -> Option<IntegerLanding> {
    let machine = input
        .program
        .machines()
        .iter()
        .find(|machine| machine.symbol == source_key.machine)?;
    let state = input
        .program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == source_key.state)?;
    let parameter = input
        .program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.symbol == parameter_symbol)?;
    integer_landing_for_type_reference(input, parameter.type_reference)
}

pub(super) fn integer_landing_for_type_reference(
    input: &InstructionSelectionInput<'_>,
    type_reference: TypeReferenceHandle,
) -> Option<IntegerLanding> {
    let landed_type = match input.program.primitive_type_reference(type_reference)? {
        PrimitiveType::I8 => LandedIntegerType::I8,
        PrimitiveType::I16 => LandedIntegerType::I16,
        PrimitiveType::I32 => LandedIntegerType::I32,
        PrimitiveType::I64 => LandedIntegerType::I64,
        PrimitiveType::U8 => LandedIntegerType::U8,
        PrimitiveType::U16 => LandedIntegerType::U16,
        PrimitiveType::U32 => LandedIntegerType::U32,
        PrimitiveType::U64 => LandedIntegerType::U64,
        PrimitiveType::Addr => LandedIntegerType::Addr,
        PrimitiveType::Bool | PrimitiveType::F32 | PrimitiveType::F64 => {
            return None;
        }
    };
    Some(IntegerLanding {
        landed_type,
        domain: input
            .program
            .type_reference_table
            .arithmetic_domain(type_reference),
    })
}

/// Alias materialization must retain the type at which a local initializer
/// landed. Stamp only the same-typed value spine: a binary's operands and
/// Mutable wrappers inherit the destination landing, while indices, call
/// arguments, and aggregate fields remain independent typing sites. An
/// explicitly landed literal (for example, a suffixed operand) is authoritative.
pub(super) fn stamp_anonymous_integer_landing_on_value_spine(
    expressions: &mut ExpressionTable,
    expression: ExpressionHandle,
    landing: IntegerLanding,
) -> ExpressionHandle {
    match expressions.expression(expression).clone() {
        ExpressionNode::Integer(literal) if literal.landing().is_none() => {
            expressions.insert(ExpressionNode::Integer(literal.with_landing(landing)))
        }
        ExpressionNode::Binary(binary) => {
            let left =
                stamp_anonymous_integer_landing_on_value_spine(expressions, binary.left, landing);
            let right =
                stamp_anonymous_integer_landing_on_value_spine(expressions, binary.right, landing);
            if left == binary.left && right == binary.right {
                expression
            } else {
                expressions.insert(ExpressionNode::Binary(TableBinaryExpression {
                    left,
                    operator: binary.operator,
                    right,
                }))
            }
        }
        ExpressionNode::Borrow(inner) => {
            let landed =
                stamp_anonymous_integer_landing_on_value_spine(expressions, inner.target, landing);
            if landed == inner.target {
                expression
            } else {
                expressions.insert(ExpressionNode::Borrow(TableBorrowExpression {
                    target: landed,
                    access: inner.access,
                }))
            }
        }
        _ => expression,
    }
}

fn local_initial_value_is_call(
    input: &InstructionSelectionInput<'_>,
    initial_value: ExpressionHandle,
) -> bool {
    let ExpressionNode::Call(call) = input.program.expression_table.expression(initial_value)
    else {
        return false;
    };
    // Borrowed-VIEW calls are NOT result-producing machine calls. They
    // materialize or preserve a descriptor from the receiver and MUST stay
    // aliased so descriptor lowering can see that receiver. Only a real
    // result-producing call (e.g. `self.idx(c)`, with its own call-result slot)
    // is excluded from aliasing here.
    !matches!(
        call.target.as_str(),
        "as_slice" | "as_mut_slice" | "as_view" | "bytes"
    )
}

fn local_requires_runtime_storage(
    input: &InstructionSelectionInput<'_>,
    source_key: StateKey,
    statement_index: usize,
    symbol: SymbolHandle,
) -> bool {
    input.state_storage.locals.iter().any(|(_, local)| {
        local.source_key == source_key
            && local.statement_index == statement_index
            && local.symbol == symbol
    })
}

fn local_is_assigned_between(
    input: &InstructionSelectionInput<'_>,
    statements: &[StatementNode],
    start: usize,
    end: usize,
    symbol: SymbolHandle,
) -> bool {
    statements
        .iter()
        .skip(start)
        .take(end.saturating_sub(start))
        .any(|statement| {
            let StatementNode::Assignment(assignment) = statement else {
                return false;
            };
            assignment_target_head_symbol(input, assignment.target) == symbol
        })
}

fn assignment_target_head_symbol(
    input: &InstructionSelectionInput<'_>,
    expression: ExpressionHandle,
) -> SymbolHandle {
    match input.program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => path.head_symbol,
        ExpressionNode::Member(member) => assignment_target_head_symbol(input, member.receiver),
        ExpressionNode::Indexed(indexed) => {
            assignment_target_head_symbol(input, indexed.collection)
        }
        ExpressionNode::Borrow(inner) => assignment_target_head_symbol(input, inner.target),
        _ => SymbolHandle::invalid(),
    }
}
