use crate::host_uses::collect_host_call_runtime_text;
use crate::places::expression_place_eq_across_tables;
use crate::slots::build_runtime_text_slots;
use crate::{
    RuntimeTextBuffer, RuntimeTextBuilder, RuntimeTextBuilderSegment,
    RuntimeTextBuilderSegmentKind, RuntimeTextPlan, RuntimeTextWrite, RuntimeTextWriteKind,
};
use omega_checked_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, ExpressionTable,
};
use omega_core::arena::{Arena, ArenaSpanInserter, HandleSpan};
use omega_platform_interface::HostCallPlan;
use omega_state_storage::StateStoragePlan;

const DEFAULT_RUNTIME_TEXT_OUTPUT_BUFFER_CAPACITY: usize = 256;

pub fn build_runtime_text_plan(
    host_calls: &HostCallPlan,
    state_storage: &StateStoragePlan,
) -> RuntimeTextPlan {
    let mut plan = RuntimeTextPlan::with_capacity(
        host_calls.calls.len(),
        host_calls
            .calls
            .len()
            .saturating_add(state_storage.mutations.len()),
        host_calls
            .calls
            .len()
            .saturating_mul(2)
            .saturating_add(state_storage.mutations.len().saturating_mul(2)),
        state_storage.mutations.len(),
        state_storage.mutations.len(),
        state_storage.mutations.len(),
    );

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
            && !is_prior_runtime_text_write_target(plan, &state_storage.expressions, mutation.target)
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

/// True when `target` was already the destination of an earlier collected text
/// write in this pass. Once a place is established as text (e.g. `self.line =
/// "prefix "`), a later `self.line = self.line + self.suffix` is a text concat
/// even though, with no string-literal anchor, it is otherwise indistinguishable
/// from a numeric add at this (type-free) layer.
fn is_prior_runtime_text_write_target(
    plan: &RuntimeTextPlan,
    expressions: &ExpressionTable,
    target: ExpressionHandle,
) -> bool {
    plan.writes.iter().any(|(_, write)| {
        write.target.is_valid()
            && expression_place_eq_across_tables(
                &plan.expressions,
                write.target,
                expressions,
                target,
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

    let builder_capacity = writes
        .iter()
        .filter(|(_, write)| write.kind == RuntimeTextWriteKind::GeneratedString)
        .count();
    let builder_segment_capacity = writes
        .iter()
        .filter(|(_, write)| write.kind == RuntimeTextWriteKind::GeneratedString)
        .map(|(_, write)| runtime_text_builder_segment_count(expressions, write.value))
        .sum();
    builders.reserve(builder_capacity);
    buffers.reserve(builder_capacity);
    builder_segments.reserve(builder_segment_capacity);

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

fn runtime_text_builder_segment_count(
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> usize {
    if let ExpressionNode::Binary(binary) = expressions.expression(expression)
        && binary.operator == BinaryOperator::Add
    {
        return runtime_text_builder_segment_count(expressions, binary.left)
            .checked_add(runtime_text_builder_segment_count(
                expressions,
                binary.right,
            ))
            .expect("runtime text builder segment count overflow");
    }

    1
}

fn collect_builder_segments(
    expressions: &ExpressionTable,
    builder_segments: &mut Arena<RuntimeTextBuilderSegment>,
    expression: ExpressionHandle,
) -> HandleSpan<RuntimeTextBuilderSegment> {
    builder_segments.insert_many_with(|segments| {
        insert_builder_segments(expressions, segments, expression);
    })
}

fn insert_builder_segments(
    expressions: &ExpressionTable,
    builder_segments: &mut ArenaSpanInserter<'_, RuntimeTextBuilderSegment>,
    expression: ExpressionHandle,
) {
    if let ExpressionNode::Binary(binary) = expressions.expression(expression)
        && binary.operator == BinaryOperator::Add
    {
        insert_builder_segments(expressions, builder_segments, binary.left);
        insert_builder_segments(expressions, builder_segments, binary.right);
        return;
    }

    builder_segments.insert(RuntimeTextBuilderSegment {
        expression,
        kind: classify_runtime_text_builder_segment(expressions, expression),
    });
}

fn classify_runtime_text_builder_segment(
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> RuntimeTextBuilderSegmentKind {
    if expressions.expression_is_stored_place(expression) {
        return RuntimeTextBuilderSegmentKind::StoredPlace;
    }

    match expressions.expression(expression) {
        ExpressionNode::String(_) => RuntimeTextBuilderSegmentKind::StaticText,
        ExpressionNode::Name(_) | ExpressionNode::Indexed(_) | ExpressionNode::Member(_) => {
            unreachable!("stored places are classified before expression node matching")
        }
        ExpressionNode::ArrayLiteral(_)
        | ExpressionNode::Binary(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::Call(_)
        | ExpressionNode::Cast(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Mutable(_)
        | ExpressionNode::Range(_)
        | ExpressionNode::StructLiteral(_)
        | ExpressionNode::Unary(_) => RuntimeTextBuilderSegmentKind::OtherExpression,
    }
}

fn is_obvious_runtime_text_value(table: &ExpressionTable, expression: ExpressionHandle) -> bool {
    if table.expression_is_stored_place(expression) {
        return true;
    }

    match table.expression(expression) {
        ExpressionNode::String(_) => true,
        ExpressionNode::Name(_) | ExpressionNode::Indexed(_) | ExpressionNode::Member(_) => {
            unreachable!("stored places are classified before expression node matching")
        }
        ExpressionNode::Binary(binary) => {
            binary.operator == BinaryOperator::Add
                && is_runtime_text_segment_like(table, binary.left)
                && is_runtime_text_segment_like(table, binary.right)
                && (contains_runtime_text_anchor(table, binary.left)
                    || contains_runtime_text_anchor(table, binary.right))
        }
        ExpressionNode::Unary(unary) => is_obvious_runtime_text_value(table, unary.operand),
        _ => false,
    }
}

fn is_runtime_text_segment_like(table: &ExpressionTable, expression: ExpressionHandle) -> bool {
    if table.expression_is_stored_place(expression) {
        return true;
    }

    match table.expression(expression) {
        ExpressionNode::String(_) => true,
        ExpressionNode::Name(_) | ExpressionNode::Indexed(_) | ExpressionNode::Member(_) => {
            unreachable!("stored places are classified before expression node matching")
        }
        ExpressionNode::Binary(binary) => {
            binary.operator == BinaryOperator::Add
                && is_runtime_text_segment_like(table, binary.left)
                && is_runtime_text_segment_like(table, binary.right)
        }
        ExpressionNode::Mutable(inner) => is_runtime_text_segment_like(table, *inner),
        ExpressionNode::Unary(unary) => is_runtime_text_segment_like(table, unary.operand),
        ExpressionNode::Range(_) => false,
        ExpressionNode::Boolean(_)
        | ExpressionNode::Call(_)
        | ExpressionNode::Cast(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_) => true,
        ExpressionNode::ArrayLiteral(_) | ExpressionNode::StructLiteral(_) => false,
    }
}

fn contains_runtime_text_anchor(table: &ExpressionTable, expression: ExpressionHandle) -> bool {
    match table.expression(expression) {
        ExpressionNode::String(_) => true,
        ExpressionNode::Binary(binary) => {
            contains_runtime_text_anchor(table, binary.left)
                || contains_runtime_text_anchor(table, binary.right)
        }
        ExpressionNode::Mutable(inner) => contains_runtime_text_anchor(table, *inner),
        ExpressionNode::Unary(unary) => contains_runtime_text_anchor(table, unary.operand),
        ExpressionNode::ArrayLiteral(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::Call(_)
        | ExpressionNode::Cast(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Indexed(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Member(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::Range(_)
        | ExpressionNode::StructLiteral(_) => false,
    }
}

fn classify_runtime_text_write(
    table: &ExpressionTable,
    expression: ExpressionHandle,
) -> RuntimeTextWriteKind {
    if table.expression_is_stored_place(expression) {
        return RuntimeTextWriteKind::StoredCopy;
    }

    match table.expression(expression) {
        ExpressionNode::String(_) => RuntimeTextWriteKind::StaticText,
        ExpressionNode::Name(_) | ExpressionNode::Indexed(_) | ExpressionNode::Member(_) => {
            unreachable!("stored places are classified before expression node matching")
        }
        ExpressionNode::Binary(_) => RuntimeTextWriteKind::GeneratedString,
        ExpressionNode::ArrayLiteral(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::Call(_)
        | ExpressionNode::Cast(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Mutable(_)
        | ExpressionNode::Range(_)
        | ExpressionNode::StructLiteral(_)
        | ExpressionNode::Unary(_) => RuntimeTextWriteKind::OtherExpression,
    }
}
