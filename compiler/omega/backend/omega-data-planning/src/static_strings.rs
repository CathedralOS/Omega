use omega_control_flow::StateKey;
use omega_runtime_branching::RuntimeBranchingCallPlan;
use omega_state_storage::{StateLocalStorage, StateStoragePlan};
use omega_state_values::StateValuePlan;
use omega_target_operations::{TargetDataObject, TargetDataObjectKind, TargetDataPlan};
use psi_checked_trees::CheckedTrees;
use psi_checked_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, ExpressionTable,
};
use psi_checked_trees::statement::StatementNode;

pub(super) fn collect_static_string_assignment_data(
    state_storage: &StateStoragePlan,
    data_plan: &mut TargetDataPlan,
) {
    for (_, mutation) in state_storage.mutations.iter() {
        if !mutation.required {
            continue;
        }

        collect_static_string_expression_data(
            &state_storage.expressions,
            mutation.value,
            mutation.source_key,
            mutation.statement_index,
            data_plan,
        );
    }
}

pub(super) fn collect_static_string_value_data(
    state_values: &StateValuePlan,
    data_plan: &mut TargetDataPlan,
) {
    for (_, value) in state_values.values.iter() {
        if !value.required {
            continue;
        }

        collect_static_string_expression_data(
            &state_values.expressions,
            value.expression,
            value.source_key,
            value.statement_index,
            data_plan,
        );
    }
}

/// String literals reachable from `let` LOCAL INITIALIZERS (`let s: [u8; 8] =
/// "hi"`, `let msg: T = T { label: "hi" }`). The storage plan records a local's
/// slot but NOT its initializer expression (that stays in the checked program),
/// so the mutation/value/branch collectors never visit these literals. Without
/// this pass the descriptor-write selection finds no data object for the
/// literal and silently skips the write, leaving the destination zeroed
/// natively while the interpreter sees the value.
pub(super) fn collect_static_string_local_initializer_data(
    program: &CheckedTrees,
    state_storage: &StateStoragePlan,
    data_plan: &mut TargetDataPlan,
) {
    for (_, local) in state_storage.locals.iter() {
        if !local.required {
            continue;
        }
        let Some(initializer) = local_initializer_expression(program, local) else {
            continue;
        };

        collect_static_string_expression_data(
            &program.expression_table,
            initializer,
            local.source_key,
            local.statement_index,
            data_plan,
        );
    }
}

/// The declared initializer of `local`, read back from the checked program's
/// statement for the local's (state, statement-index) coordinates.
pub(super) fn local_initializer_expression(
    program: &CheckedTrees,
    local: &StateLocalStorage,
) -> Option<ExpressionHandle> {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == local.source_key.machine)?;
    let state = program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == local.source_key.state)?;
    let statement = program
        .statement_table
        .statements(state.statement_nodes)
        .get(local.statement_index)?;
    let StatementNode::LocalData(local_data) = statement else {
        return None;
    };
    local_data
        .initial_value
        .is_valid()
        .then_some(local_data.initial_value)
}

pub(super) fn collect_static_string_branch_target_data(
    runtime_branching: &RuntimeBranchingCallPlan,
    data_plan: &mut TargetDataPlan,
) {
    for (_, expansion) in runtime_branching.leaf_expansions.iter() {
        if !expansion.target_value.is_valid() {
            continue;
        }

        collect_static_string_expression_data(
            &runtime_branching.expressions,
            expansion.target_value,
            expansion.branch_key,
            expansion.target_statement_index,
            data_plan,
        );
    }

    for (_, expansion) in runtime_branching.straight_line_expansions.iter() {
        if !expansion.target_value.is_valid() {
            continue;
        }

        collect_static_string_expression_data(
            &runtime_branching.expressions,
            expansion.target_value,
            expansion.source_key,
            expansion.statement_index,
            data_plan,
        );
    }
}

fn collect_static_string_expression_data(
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
    source_key: StateKey,
    source_statement: usize,
    data_plan: &mut TargetDataPlan,
) {
    match expressions.expression(expression) {
        ExpressionNode::Atomic(atomic) => collect_static_string_expression_data(
            expressions,
            atomic.value,
            source_key,
            source_statement,
            data_plan,
        ),
        ExpressionNode::String(value) => {
            let offset = data_plan.bytes.len();
            let byte_span = if value.is_empty() {
                data_plan.bytes.insert_many(std::iter::once(0))
            } else {
                let span = data_plan.bytes.insert_many(value.iter().copied());
                // NUL-terminate for C-string consumers (`_open`/`_unlink` path
                // arguments). The terminator is inserted right after the content
                // but is NOT part of the object's span, so length-based consumers
                // (write_bytes, Console write) still see only the content length.
                data_plan.bytes.insert_many(std::iter::once(0));
                span
            };
            let symbol_index = data_plan.objects.len() + 1;

            data_plan.objects.insert(TargetDataObject {
                symbol: format!("omega_string_literal_{symbol_index}").into(),
                kind: TargetDataObjectKind::StaticString,
                offset,
                bytes: byte_span,
                alignment: 1,
                source_key,
                source_statement,
            });
        }
        ExpressionNode::StructLiteral(struct_literal) => {
            for field in expressions.struct_fields(struct_literal.fields) {
                collect_static_string_expression_data(
                    expressions,
                    field.value,
                    source_key,
                    source_statement,
                    data_plan,
                );
            }
        }
        ExpressionNode::ArrayLiteral(elements) => {
            for element in expressions.expression_handles(*elements) {
                collect_static_string_expression_data(
                    expressions,
                    *element,
                    source_key,
                    source_statement,
                    data_plan,
                );
            }
        }
        ExpressionNode::Range(range) => {
            collect_static_string_expression_data(
                expressions,
                range.start,
                source_key,
                source_statement,
                data_plan,
            );
            collect_static_string_expression_data(
                expressions,
                range.end,
                source_key,
                source_statement,
                data_plan,
            );
        }
        ExpressionNode::Binary(binary) => {
            // An all-literal concat (`"prefix " + "omega"`) is written as ONE
            // folded literal (the runtime-text planner folds the write's
            // value), so the descriptor write looks up a data object holding
            // the folded bytes. Plan that object alongside the per-segment
            // literals (which non-folded builder shapes still reference).
            if let Some(folded) = fold_static_string_expression(expressions, expression) {
                let offset = data_plan.bytes.len();
                let byte_span = data_plan.bytes.insert_many(folded.iter().copied());
                let symbol_index = data_plan.objects.len() + 1;
                data_plan.objects.insert(TargetDataObject {
                    symbol: format!("omega_string_literal_{symbol_index}").into(),
                    kind: TargetDataObjectKind::StaticString,
                    offset,
                    bytes: byte_span,
                    alignment: 1,
                    source_key,
                    source_statement,
                });
            }
            collect_static_string_expression_data(
                expressions,
                binary.left,
                source_key,
                source_statement,
                data_plan,
            );
            collect_static_string_expression_data(
                expressions,
                binary.right,
                source_key,
                source_statement,
                data_plan,
            );
        }
        ExpressionNode::Unary(unary) => {
            collect_static_string_expression_data(
                expressions,
                unary.operand,
                source_key,
                source_statement,
                data_plan,
            );
        }
        ExpressionNode::Call(call) => {
            if call.receiver.is_valid() {
                collect_static_string_expression_data(
                    expressions,
                    call.receiver,
                    source_key,
                    source_statement,
                    data_plan,
                );
            }

            for argument in expressions.expression_handles(call.arguments) {
                collect_static_string_expression_data(
                    expressions,
                    *argument,
                    source_key,
                    source_statement,
                    data_plan,
                );
            }
        }
        ExpressionNode::Cast(cast) => {
            collect_static_string_expression_data(
                expressions,
                cast.value,
                source_key,
                source_statement,
                data_plan,
            );
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Indexed(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Member(_)
        | ExpressionNode::Mutable(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::ZeroValue(_) => {}
    }
}

/// Fold an all-literal `+` tree to the single string it denotes; `None` when
/// any leaf is not a string literal. Mirrors the runtime-text planner's fold
/// so the data object it plans here matches the write value byte-for-byte.
fn fold_static_string_expression(
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<Vec<u8>> {
    match expressions.expression(expression) {
        ExpressionNode::String(value) => Some(value.to_vec()),
        ExpressionNode::Binary(binary) if binary.operator == BinaryOperator::Add => {
            let mut folded = fold_static_string_expression(expressions, binary.left)?;
            folded.extend_from_slice(&fold_static_string_expression(expressions, binary.right)?);
            Some(folded)
        }
        _ => None,
    }
}
