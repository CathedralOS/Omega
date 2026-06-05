use super::argument_materialization::{
    select_runtime_dispatch_argument_materialization, static_runtime_argument_value,
};
use super::guards::{
    select_runtime_dispatch_expression_guard,
    select_runtime_dispatch_expression_guard_conjuncts_in_table,
    select_runtime_dispatch_expression_guard_in_table,
};
use crate::InstructionSelectionInput;
use crate::selection::bindings::RuntimeAliasBinding;
use crate::selection::storage_places::{
    resolve_runtime_storage_is_signed_in_table, resolve_runtime_storage_primitive_type_in_table,
};
use omega_checked_trees::expression::{BinaryOperator, ExpressionNode, ExpressionTable};
use omega_checked_trees::types::PrimitiveType;
use omega_checked_trees::statement::TransitionGuard;
use omega_control_flow::StateKey;
use omega_core::arena::Arena;
use omega_runtime_dispatch_loop::{RuntimeDispatchLoopAction, RuntimeDispatchLoopEdge};
use omega_state_guards::{StateGuardOperandStorage, lower_guard_conjunction};

use crate::selection::instruction_sink::SelectedInstructionSink;
use omega_abstract_operations::{
    RuntimeStorageRegion, RuntimeValueOperand, SelectedInstruction, SelectedInstructionKind,
    StateGuardLowering, StateGuardOperator,
};

/// Whether `edge`'s guard is a single ordered comparison (`<`, `<=`, `>`, `>=`)
/// whose operands are an unsigned integer type. Such a guard must branch with
/// unsigned conditions; the clause operator is swapped accordingly. And-
/// conjunctions and signed/undeterminable operands keep the signed form.
fn guard_comparison_operands_unsigned(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    edge: &RuntimeDispatchLoopEdge,
) -> bool {
    if !edge.guard_has_expression {
        return false;
    }
    let expressions = &input.state_guards.expressions;
    let ExpressionNode::Binary(binary) = expressions.expression(edge.guard_expression) else {
        return false;
    };
    if !matches!(
        binary.operator,
        BinaryOperator::Greater
            | BinaryOperator::GreaterOrEqual
            | BinaryOperator::Less
            | BinaryOperator::LessOrEqual
    ) {
        return false;
    }
    let signed =
        resolve_runtime_storage_is_signed_in_table(input, dispatch_index, source_key, expressions, binary.left)
            .or_else(|| {
                resolve_runtime_storage_is_signed_in_table(
                    input,
                    dispatch_index,
                    source_key,
                    expressions,
                    binary.right,
                )
            });
    signed == Some(false)
}

/// True when a guard comparison's operands are f64, so the static/runtime
/// compare must use `comisd` rather than an integer `cmp`. First cut: f64 only
/// (matches the arithmetic path). The operand type is read from whichever side
/// resolves to a storage place (the literal side does not).
fn guard_comparison_operands_float(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    edge: &RuntimeDispatchLoopEdge,
) -> bool {
    if !edge.guard_has_expression {
        return false;
    }
    let expressions = &input.state_guards.expressions;
    let ExpressionNode::Binary(binary) = expressions.expression(edge.guard_expression) else {
        return false;
    };
    if !matches!(
        binary.operator,
        BinaryOperator::Equal
            | BinaryOperator::NotEqual
            | BinaryOperator::Greater
            | BinaryOperator::GreaterOrEqual
            | BinaryOperator::Less
            | BinaryOperator::LessOrEqual
    ) {
        return false;
    }
    let primitive = resolve_runtime_storage_primitive_type_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        binary.left,
    )
    .or_else(|| {
        resolve_runtime_storage_primitive_type_in_table(
            input,
            dispatch_index,
            source_key,
            expressions,
            binary.right,
        )
    });
    matches!(primitive, Some(PrimitiveType::F64))
}

fn unsigned_comparison_operator(operator: StateGuardOperator) -> StateGuardOperator {
    match operator {
        StateGuardOperator::Greater => StateGuardOperator::GreaterUnsigned,
        StateGuardOperator::GreaterOrEqual => StateGuardOperator::GreaterOrEqualUnsigned,
        StateGuardOperator::Less => StateGuardOperator::LessUnsigned,
        StateGuardOperator::LessOrEqual => StateGuardOperator::LessOrEqualUnsigned,
        other => other,
    }
}

pub(super) fn select_runtime_dispatch_edge(
    input: &InstructionSelectionInput<'_>,
    edge: &RuntimeDispatchLoopEdge,
    source_key: StateKey,
    source_dispatch_index: u32,
    aliases: &[RuntimeAliasBinding],
    alias_expressions: &ExpressionTable,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) {
    if matches!(edge.action, RuntimeDispatchLoopAction::Unknown) {
        return;
    }

    select_dispatch_guard_instructions(
        input,
        edge,
        source_key,
        source_dispatch_index,
        runtime_value_operands,
        selected_instructions,
    );

    match edge.action {
        RuntimeDispatchLoopAction::EnterState => {
            select_runtime_dispatch_argument_materialization(
                input,
                source_key,
                source_dispatch_index,
                edge.statement_index,
                edge.target_dispatch_index,
                edge.target_arguments,
                aliases,
                alias_expressions,
                runtime_value_operands,
                selected_instructions,
            );

            selected_instructions.push(SelectedInstruction {
                kind: SelectedInstructionKind::SetDispatchState {
                    dispatch_index: edge.target_dispatch_index,
                },
                source_key,
                source_statement: edge.statement_index,
            });
        }
        RuntimeDispatchLoopAction::Terminate => {
            select_runtime_dispatch_return_value(input, edge, source_key, selected_instructions);
            selected_instructions.push(SelectedInstruction {
                kind: SelectedInstructionKind::TerminateDispatch,
                source_key,
                source_statement: edge.statement_index,
            });
        }
        RuntimeDispatchLoopAction::Unknown => {}
    }
}

fn select_runtime_dispatch_return_value(
    input: &InstructionSelectionInput<'_>,
    edge: &RuntimeDispatchLoopEdge,
    source_key: StateKey,
    selected_instructions: &mut SelectedInstructionSink,
) {
    let Some(value) = static_terminal_target_value(input, source_key, edge.order) else {
        return;
    };
    selected_instructions.push(SelectedInstruction {
        kind: SelectedInstructionKind::WriteReturnRegisterInteger {
            byte_size: 4,
            value,
        },
        source_key,
        source_statement: edge.statement_index,
    });
}

fn static_terminal_target_value(
    input: &InstructionSelectionInput<'_>,
    source_key: StateKey,
    edge_order: usize,
) -> Option<i64> {
    let state = input.control_flow.state_by_key(source_key)?;
    let transition = input
        .control_flow
        .transitions
        .span(state.transitions)?
        .get(edge_order)?;
    let value = transition.expressions.target_value;
    if !value.is_valid() {
        return None;
    }

    static_runtime_argument_value(input.control_flow.expressions.expression(value))
}

fn select_dispatch_guard_instructions(
    input: &InstructionSelectionInput<'_>,
    edge: &RuntimeDispatchLoopEdge,
    source_key: StateKey,
    source_dispatch_index: u32,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) {
    if !guard_can_emit_directly(edge) {
        let clauses = lower_guard_conjunction(
            input.state_guards,
            input.layouts,
            input.runtime_storage,
            input.entry_key.machine,
            source_key,
            source_key.machine,
            source_dispatch_index,
            edge.order,
        );
        if !clauses.is_empty() {
            let unsigned = guard_comparison_operands_unsigned(
                input,
                source_dispatch_index,
                source_key,
                edge,
            );
            for clause in clauses.iter().copied() {
                let operator = if unsigned {
                    unsigned_comparison_operator(clause.operator)
                } else {
                    clause.operator
                };
                let kind = if matches!(clause.lowering, StateGuardLowering::CompareRuntimeValue)
                    && clause.has_storage
                    && clause.has_right_storage
                {
                    SelectedInstructionKind::CompareRuntimeStorage {
                        left_region: guard_storage_region(clause.storage),
                        left_offset: clause.byte_offset,
                        right_region: guard_storage_region(clause.right_storage),
                        right_offset: clause.right_byte_offset,
                        byte_size: clause.byte_size,
                        operator,
                        // Float operands inside an AND-conjunction clause are a
                        // follow-on; single comparisons take the path below.
                        is_float: false,
                    }
                } else {
                    SelectedInstructionKind::EvaluateDispatchGuard {
                        guard_lowering: clause.lowering,
                        operator,
                        storage_region: guard_storage_region(clause.storage),
                        byte_offset: clause.byte_offset,
                        byte_size: clause.byte_size,
                        expected_value: clause.expected_value,
                        has_storage: clause.has_storage,
                        // Float comparisons inside an AND-conjunction clause are a
                        // follow-on; single-comparison guards take the path below.
                        is_float: false,
                    }
                };
                selected_instructions.push(SelectedInstruction {
                    kind,
                    source_key,
                    source_statement: edge.statement_index,
                });
            }
            return;
        }
    }

    if !guard_can_emit_directly(edge) {
        if edge.guard_has_expression {
            let guards = select_runtime_dispatch_expression_guard_conjuncts_in_table(
                input,
                source_dispatch_index,
                source_key,
                edge.statement_index,
                &input.state_guards.expressions,
                edge.guard_expression,
                runtime_value_operands,
            );
            if !guards.is_empty() {
                for kind in guards {
                    selected_instructions.push(SelectedInstruction {
                        kind,
                        source_key,
                        source_statement: edge.statement_index,
                    });
                }
                return;
            }
        }

        if edge.guard_has_expression
            && let Some(kind) = select_runtime_dispatch_expression_guard_in_table(
                input,
                source_dispatch_index,
                source_key,
                edge.statement_index,
                &input.state_guards.expressions,
                edge.guard_expression,
                runtime_value_operands,
            )
        {
            selected_instructions.push(SelectedInstruction {
                kind,
                source_key,
                source_statement: edge.statement_index,
            });
            return;
        }

        let guard = transition_guard_for_edge(input, edge);
        if let Some(kind) = select_runtime_dispatch_expression_guard(
            input,
            source_dispatch_index,
            source_key,
            edge.statement_index,
            &guard,
            runtime_value_operands,
        ) {
            selected_instructions.push(SelectedInstruction {
                kind,
                source_key,
                source_statement: edge.statement_index,
            });
            return;
        }
    }

    let is_float = guard_comparison_operands_float(input, source_dispatch_index, source_key, edge);
    // `ucomisd` sets CF/ZF exactly like an unsigned integer `cmp`, so a float
    // ordering comparison must use the unsigned failure-branch conditions
    // (jae/jbe/ja/jb), not the signed ones — note F64::is_signed_integer() is
    // true, so the unsigned-operand check below does NOT cover floats. Equal/
    // NotEqual are unaffected by the unsigned swap (they stay je/jne).
    let operator = if is_float
        || guard_comparison_operands_unsigned(input, source_dispatch_index, source_key, edge)
    {
        unsigned_comparison_operator(edge.guard_operator)
    } else {
        edge.guard_operator
    };
    let guard_instruction = match edge.guard_lowering {
        StateGuardLowering::CompareRuntimeValue
            if edge.guard_has_storage && edge.guard_has_right_storage =>
        {
            SelectedInstructionKind::CompareRuntimeStorage {
                left_region: guard_storage_region(edge.guard_storage),
                left_offset: edge.guard_byte_offset,
                right_region: guard_storage_region(edge.guard_right_storage),
                right_offset: edge.guard_right_byte_offset,
                byte_size: edge.guard_byte_size,
                operator,
                is_float,
            }
        }
        _ => SelectedInstructionKind::EvaluateDispatchGuard {
            guard_lowering: edge.guard_lowering,
            operator,
            storage_region: guard_storage_region(edge.guard_storage),
            byte_offset: edge.guard_byte_offset,
            byte_size: edge.guard_byte_size,
            expected_value: edge.guard_expected_value,
            has_storage: edge.guard_has_storage,
            is_float,
        },
    };
    selected_instructions.push(SelectedInstruction {
        kind: guard_instruction,
        source_key,
        source_statement: edge.statement_index,
    });
}

fn transition_guard_for_edge(
    input: &InstructionSelectionInput<'_>,
    edge: &RuntimeDispatchLoopEdge,
) -> TransitionGuard {
    if edge.guard_has_expression {
        TransitionGuard::When(
            input
                .state_guards
                .expressions
                .to_tree(edge.guard_expression),
        )
    } else {
        TransitionGuard::Always
    }
}

fn guard_can_emit_directly(edge: &RuntimeDispatchLoopEdge) -> bool {
    match edge.guard_lowering {
        // ForwardBranchSkip / BranchArmsEnd never appear as a dispatch-edge guard
        // (leaf-arm only); treat them as trivially emittable.
        StateGuardLowering::NoOp
        | StateGuardLowering::ForwardBranchSkip
        | StateGuardLowering::BranchArmsEnd => true,
        StateGuardLowering::CompareStaticValue => {
            edge.guard_has_storage
                && matches!(
                    edge.guard_operator,
                    omega_abstract_operations::StateGuardOperator::Equal
                        | omega_abstract_operations::StateGuardOperator::NotEqual
                        | omega_abstract_operations::StateGuardOperator::Greater
                        | omega_abstract_operations::StateGuardOperator::GreaterOrEqual
                        | omega_abstract_operations::StateGuardOperator::Less
                        | omega_abstract_operations::StateGuardOperator::LessOrEqual
                )
                && matches!(edge.guard_byte_size, 1 | 4 | 8)
        }
        StateGuardLowering::CompareRuntimeValue => {
            edge.guard_has_storage
                && edge.guard_has_right_storage
                && matches!(
                    edge.guard_operator,
                    omega_abstract_operations::StateGuardOperator::Equal
                        | omega_abstract_operations::StateGuardOperator::NotEqual
                        | omega_abstract_operations::StateGuardOperator::Greater
                        | omega_abstract_operations::StateGuardOperator::GreaterOrEqual
                        | omega_abstract_operations::StateGuardOperator::Less
                        | omega_abstract_operations::StateGuardOperator::LessOrEqual
                )
                && matches!(edge.guard_byte_size, 1 | 4 | 8)
        }
        StateGuardLowering::NeedsRuntimeExpression => false,
    }
}

fn guard_storage_region(storage: StateGuardOperandStorage) -> RuntimeStorageRegion {
    match storage {
        StateGuardOperandStorage::MachineOwned | StateGuardOperandStorage::Unknown => {
            RuntimeStorageRegion::Machine
        }
        StateGuardOperandStorage::RuntimeFrame => RuntimeStorageRegion::RuntimeFrame,
    }
}
