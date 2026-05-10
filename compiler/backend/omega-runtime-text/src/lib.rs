mod host_uses;
mod model;
pub mod places;
mod slots;

use host_uses::collect_host_call_runtime_text;
pub use model::{
    RuntimeTextBuffer, RuntimeTextBuilder, RuntimeTextBuilderSegment,
    RuntimeTextBuilderSegmentKind, RuntimeTextPlan, RuntimeTextSlot, RuntimeTextSource,
    RuntimeTextUse, RuntimeTextWrite, RuntimeTextWriteKind,
};
use omega_platform_interface::HostCallPlan;
use omega_state_storage::StateStoragePlan;
use omega_typed_program::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, ExpressionTable,
};
use slots::build_runtime_text_slots;

pub fn build_runtime_text_plan(
    host_calls: &HostCallPlan,
    state_storage: &StateStoragePlan,
) -> RuntimeTextPlan {
    let mut plan = RuntimeTextPlan::default();

    for (_, host_call) in host_calls.calls.iter() {
        collect_host_call_runtime_text(host_calls, host_call, &mut plan);
    }
    collect_runtime_text_writes(state_storage, &mut plan);
    collect_runtime_text_builders(&mut plan);
    plan.slots = build_runtime_text_slots(&mut plan);

    plan
}

fn collect_runtime_text_writes(state_storage: &StateStoragePlan, plan: &mut RuntimeTextPlan) {
    for (_, mutation) in state_storage.mutations.iter() {
        if !is_text_place(&state_storage.expressions, mutation.target) {
            continue;
        }
        let target = plan
            .expressions
            .copy_from(&state_storage.expressions, mutation.target);
        let value = plan
            .expressions
            .copy_from(&state_storage.expressions, mutation.value);

        plan.writes.insert(RuntimeTextWrite {
            source_key: mutation.source_key,
            statement_index: mutation.statement_index,
            kind: classify_runtime_text_write(&plan.expressions, value),
            target,
            value,
        });
    }
}

fn collect_runtime_text_builders(plan: &mut RuntimeTextPlan) {
    let writes = plan
        .writes
        .iter()
        .map(|(_, write)| write.clone())
        .collect::<Vec<_>>();

    for write in writes {
        if write.kind != RuntimeTextWriteKind::GeneratedString {
            continue;
        }

        let mut segments = Vec::new();
        collect_builder_segments(plan, write.value, &mut segments);
        let segment_span = plan.builder_segments.insert_many(segments);
        plan.builders.insert(RuntimeTextBuilder {
            source_key: write.source_key,
            statement_index: write.statement_index,
            target: write.target,
            segments: segment_span,
        });
    }
}

fn collect_builder_segments(
    plan: &RuntimeTextPlan,
    expression: ExpressionHandle,
    segments: &mut Vec<RuntimeTextBuilderSegment>,
) {
    if let ExpressionNode::Binary(binary) = plan.expressions.expression(expression)
        && binary.operator == BinaryOperator::Add
    {
        collect_builder_segments(plan, binary.left, segments);
        collect_builder_segments(plan, binary.right, segments);
        return;
    }

    segments.push(RuntimeTextBuilderSegment {
        expression,
        kind: classify_runtime_text_builder_segment(plan, expression),
    });
}

fn classify_runtime_text_builder_segment(
    plan: &RuntimeTextPlan,
    expression: ExpressionHandle,
) -> RuntimeTextBuilderSegmentKind {
    match plan.expressions.expression(expression) {
        ExpressionNode::String(_) => RuntimeTextBuilderSegmentKind::StaticText,
        ExpressionNode::Name(_) | ExpressionNode::Indexed(_) => {
            RuntimeTextBuilderSegmentKind::StoredPlace
        }
        ExpressionNode::ArrayLiteral(_)
        | ExpressionNode::Binary(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Mutable(_)
        | ExpressionNode::StructLiteral(_) => RuntimeTextBuilderSegmentKind::OtherExpression,
    }
}

fn is_text_place(table: &ExpressionTable, expression: ExpressionHandle) -> bool {
    match table.expression(expression) {
        ExpressionNode::Name(path) => table
            .name_path_members(path.members)
            .last()
            .is_some_and(|segment| segment == "text"),
        ExpressionNode::Indexed(indexed) => is_text_place(table, indexed.collection),
        ExpressionNode::Mutable(expression) => is_text_place(table, *expression),
        _ => false,
    }
}

fn classify_runtime_text_write(
    table: &ExpressionTable,
    expression: ExpressionHandle,
) -> RuntimeTextWriteKind {
    match table.expression(expression) {
        ExpressionNode::String(_) => RuntimeTextWriteKind::StaticText,
        ExpressionNode::Name(_) | ExpressionNode::Indexed(_) => RuntimeTextWriteKind::StoredCopy,
        ExpressionNode::Binary(_) => RuntimeTextWriteKind::GeneratedString,
        ExpressionNode::ArrayLiteral(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Mutable(_)
        | ExpressionNode::StructLiteral(_) => RuntimeTextWriteKind::OtherExpression,
    }
}
