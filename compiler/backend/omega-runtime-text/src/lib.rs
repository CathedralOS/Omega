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
use omega_checked_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, ExpressionTable,
};
use omega_core::arena::{Arena, Handle, HandleSpan};
use omega_platform_interface::HostCallPlan;
use omega_state_storage::StateStoragePlan;
use places::expression_place_eq_across_tables;
use slots::build_runtime_text_slots;

const DEFAULT_RUNTIME_TEXT_OUTPUT_BUFFER_CAPACITY: usize = 256;

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
        if !is_known_runtime_text_place(plan, &state_storage.expressions, mutation.target)
            && !is_obvious_runtime_text_value(&state_storage.expressions, mutation.value)
        {
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

fn is_known_runtime_text_place(
    plan: &RuntimeTextPlan,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> bool {
    plan.buffers.iter().any(|(_, buffer)| {
        buffer.text_place.is_valid()
            && expression_place_eq_across_tables(
                &plan.expressions,
                buffer.text_place,
                expressions,
                expression,
            )
    })
}

fn collect_runtime_text_builders(plan: &mut RuntimeTextPlan) {
    let RuntimeTextPlan {
        expressions,
        writes,
        buffers,
        builders,
        builder_segments,
        ..
    } = plan;

    for (_, write) in writes.iter() {
        if write.kind != RuntimeTextWriteKind::GeneratedString {
            continue;
        }

        let segment_span = collect_builder_segments(expressions, builder_segments, write.value);
        builders.insert(RuntimeTextBuilder {
            source_key: write.source_key,
            statement_index: write.statement_index,
            target: write.target,
            segments: segment_span,
        });
        buffers.insert(RuntimeTextBuffer {
            source_key: write.source_key,
            statement_index: write.statement_index,
            target: write.target,
            text_place: write.target,
            byte_capacity: DEFAULT_RUNTIME_TEXT_OUTPUT_BUFFER_CAPACITY,
        });
    }
}

fn collect_builder_segments(
    expressions: &ExpressionTable,
    builder_segments: &mut Arena<RuntimeTextBuilderSegment>,
    expression: ExpressionHandle,
) -> HandleSpan<RuntimeTextBuilderSegment> {
    let mut start = Handle::invalid();
    let mut count = 0u32;

    append_builder_segments(
        expressions,
        builder_segments,
        expression,
        &mut start,
        &mut count,
    );

    if count == 0 {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(start, count)
    }
}

fn append_builder_segments(
    expressions: &ExpressionTable,
    builder_segments: &mut Arena<RuntimeTextBuilderSegment>,
    expression: ExpressionHandle,
    start: &mut Handle<RuntimeTextBuilderSegment>,
    count: &mut u32,
) {
    if let ExpressionNode::Binary(binary) = expressions.expression(expression)
        && binary.operator == BinaryOperator::Add
    {
        append_builder_segments(expressions, builder_segments, binary.left, start, count);
        append_builder_segments(expressions, builder_segments, binary.right, start, count);
        return;
    }

    let handle = builder_segments.append(RuntimeTextBuilderSegment {
        expression,
        kind: classify_runtime_text_builder_segment(expressions, expression),
    });
    if *count == 0 {
        *start = handle;
    }
    *count = count
        .checked_add(1)
        .expect("runtime text builder segment span count overflow");
}

fn classify_runtime_text_builder_segment(
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> RuntimeTextBuilderSegmentKind {
    match expressions.expression(expression) {
        ExpressionNode::String(_) => RuntimeTextBuilderSegmentKind::StaticText,
        ExpressionNode::Name(_) | ExpressionNode::Indexed(_) | ExpressionNode::Member(_) => {
            RuntimeTextBuilderSegmentKind::StoredPlace
        }
        ExpressionNode::ArrayLiteral(_)
        | ExpressionNode::Binary(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::Call(_)
        | ExpressionNode::Cast(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Mutable(_)
        | ExpressionNode::StructLiteral(_) => RuntimeTextBuilderSegmentKind::OtherExpression,
    }
}

fn is_obvious_runtime_text_value(table: &ExpressionTable, expression: ExpressionHandle) -> bool {
    match table.expression(expression) {
        ExpressionNode::String(_) => true,
        ExpressionNode::Binary(binary) => {
            binary.operator == BinaryOperator::Add
                && is_runtime_text_segment_like(table, binary.left)
                && is_runtime_text_segment_like(table, binary.right)
                && (contains_runtime_text_anchor(table, binary.left)
                    || contains_runtime_text_anchor(table, binary.right))
        }
        _ => false,
    }
}

fn is_runtime_text_segment_like(table: &ExpressionTable, expression: ExpressionHandle) -> bool {
    match table.expression(expression) {
        ExpressionNode::String(_) => true,
        ExpressionNode::Name(_) | ExpressionNode::Indexed(_) | ExpressionNode::Member(_) => true,
        ExpressionNode::Binary(binary) => {
            binary.operator == BinaryOperator::Add
                && is_runtime_text_segment_like(table, binary.left)
                && is_runtime_text_segment_like(table, binary.right)
        }
        ExpressionNode::Call(call) => call.target.as_str() == "to_string",
        ExpressionNode::Mutable(inner) => is_runtime_text_segment_like(table, *inner),
        ExpressionNode::ArrayLiteral(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::Cast(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::StructLiteral(_) => false,
    }
}

fn contains_runtime_text_anchor(table: &ExpressionTable, expression: ExpressionHandle) -> bool {
    match table.expression(expression) {
        ExpressionNode::String(_) => true,
        ExpressionNode::Binary(binary) => {
            contains_runtime_text_anchor(table, binary.left)
                || contains_runtime_text_anchor(table, binary.right)
        }
        ExpressionNode::Call(call) => call.target.as_str() == "to_string",
        ExpressionNode::Mutable(inner) => contains_runtime_text_anchor(table, *inner),
        ExpressionNode::ArrayLiteral(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::Cast(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Indexed(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Member(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::StructLiteral(_) => false,
    }
}

fn classify_runtime_text_write(
    table: &ExpressionTable,
    expression: ExpressionHandle,
) -> RuntimeTextWriteKind {
    match table.expression(expression) {
        ExpressionNode::String(_) => RuntimeTextWriteKind::StaticText,
        ExpressionNode::Name(_) | ExpressionNode::Indexed(_) | ExpressionNode::Member(_) => {
            RuntimeTextWriteKind::StoredCopy
        }
        ExpressionNode::Binary(_) => RuntimeTextWriteKind::GeneratedString,
        ExpressionNode::ArrayLiteral(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::Call(_)
        | ExpressionNode::Cast(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Mutable(_)
        | ExpressionNode::StructLiteral(_) => RuntimeTextWriteKind::OtherExpression,
    }
}
