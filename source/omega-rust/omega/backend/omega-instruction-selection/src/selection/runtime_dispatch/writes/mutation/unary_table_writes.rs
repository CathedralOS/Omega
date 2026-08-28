use crate::InstructionSelectionInput;
use crate::selection::storage_places::resolve_runtime_storage_place_in_table;
use omega_abstract_operations::{
    RuntimeStorageRegion, RuntimeValueOperand, SelectedInstructionKind, StateGuardOperator,
};
use omega_control_flow::StateKey;
use psi_arena::Arena;
use psi_checked_trees::expression::{
    ExpressionHandle, ExpressionNode, ExpressionTable, UnaryOperator,
};

/// Materialize a checked runtime logical-NOT expression into an exact byte-sized
/// target. Keeping this independent of a particular target kind lets ordinary
/// locals, spliced call-result slots, and process-entry scratch share one
/// lowering instead of each growing a terminal-shape permutation.
#[allow(clippy::too_many_arguments)]
pub(in crate::selection) fn select_runtime_logical_not_write_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    target_region: RuntimeStorageRegion,
    target_offset: usize,
    target_byte_size: usize,
    value: ExpressionHandle,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
) -> Option<SelectedInstructionKind> {
    let ExpressionNode::Unary(unary) = expressions.expression(value) else {
        return None;
    };
    if unary.operator != UnaryOperator::LogicalNot || target_byte_size != 1 {
        return None;
    }
    let place = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        unary.operand,
    )?;
    if place.byte_count != 1 {
        return None;
    }
    let left = runtime_value_operands.insert(RuntimeValueOperand::Storage {
        region: place.region,
        byte_offset: place.byte_offset,
        byte_size: place.byte_count,
    });
    let right = runtime_value_operands.insert(RuntimeValueOperand::Immediate(0));
    Some(
        crate::selection::runtime_dispatch::write_place_binary_direct(
            target_region,
            target_offset,
            target_byte_size,
            left,
            StateGuardOperator::Equal,
            right,
            false,
            psi_numerics::arithmetic::ArithmeticDomain::Exact,
            false,
        ),
    )
}
