use crate::host_uses::collect_host_call_runtime_text;
use crate::places::expression_place_eq_across_tables;
use crate::slots::build_runtime_text_slots;
use crate::{
    RuntimeTextBuffer, RuntimeTextBuilder, RuntimeTextBuilderSegment,
    RuntimeTextBuilderSegmentKind, RuntimeTextPlan, RuntimeTextWrite, RuntimeTextWriteKind,
};
use omega_platform_interface::HostCallPlan;
use omega_state_storage::StateStoragePlan;
use psi_arena::{Arena, ArenaSpanInserter, HandleSpan};
use psi_checked_trees::expression::{
    BinaryOperator, Expression, ExpressionHandle, ExpressionNode, ExpressionTable, NamePath,
};

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
    collect_runtime_text_local_initializer_writes(state_storage, &mut plan);
    collect_runtime_text_builders(&mut plan);
    plan.slots = build_runtime_text_slots(&mut plan);

    plan
}

fn collect_runtime_text_writes(state_storage: &StateStoragePlan, plan: &mut RuntimeTextPlan) {
    for (_, mutation) in state_storage.mutations.iter() {
        if !is_known_runtime_text_place(plan, &state_storage.expressions, mutation.target)
            && !is_prior_runtime_text_write_target(
                plan,
                &state_storage.expressions,
                mutation.target,
            )
            && !is_obvious_runtime_text_value(&state_storage.expressions, mutation.value)
        {
            continue;
        }
        let target = plan
            .expressions
            .copy_from(&state_storage.expressions, mutation.target);
        let value = copy_text_value_folding_static_concat(
            &state_storage.expressions,
            mutation.value,
            &mut plan.expressions,
        );

        plan.writes.insert(RuntimeTextWrite {
            source_key: mutation.source_key,
            statement_index: mutation.statement_index,
            kind: classify_runtime_text_write(&plan.expressions, value),
            target,
            value,
        });
    }
}

/// Copy a text write's VALUE into the plan, folding an all-literal concat
/// (`"prefix " + "omega"`) into the single literal it denotes. Left unfolded,
/// such a value classifies as a GeneratedString builder whose per-segment
/// emission OVERWRITES indexed targets segment by segment (each machine-indexed
/// "append" is a full descriptor write), leaving only the last segment's bytes
/// behind. Folded, it is an ordinary StaticText write with one data object.
fn copy_text_value_folding_static_concat(
    source_expressions: &ExpressionTable,
    value: ExpressionHandle,
    plan_expressions: &mut ExpressionTable,
) -> ExpressionHandle {
    if matches!(
        source_expressions.expression(value),
        ExpressionNode::Binary(_)
    ) && let Some(folded) = fold_static_text_value(source_expressions, value)
    {
        return plan_expressions.insert(ExpressionNode::String(folded.into()));
    }
    plan_expressions.copy_from(source_expressions, value)
}

fn fold_static_text_value(
    table: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<Vec<u8>> {
    match table.expression(expression) {
        ExpressionNode::String(value) => Some(value.to_vec()),
        ExpressionNode::Binary(binary) if binary.operator == BinaryOperator::Add => {
            let mut folded = fold_static_text_value(table, binary.left)?;
            folded.extend_from_slice(&fold_static_text_value(table, binary.right)?);
            Some(folded)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use psi_checked_trees::expression::TableBinaryExpression;
    use std::sync::Arc;

    #[test]
    fn static_concat_fold_retains_non_utf8_bytes() {
        let mut expressions = ExpressionTable::default();
        let left = expressions.insert(ExpressionNode::String(Arc::from(&[0x80][..])));
        let right = expressions.insert(ExpressionNode::String(Arc::from(&b"A"[..])));
        let concat = expressions.insert(ExpressionNode::Binary(TableBinaryExpression {
            left,
            operator: BinaryOperator::Add,
            right,
        }));

        assert_eq!(
            fold_static_text_value(&expressions, concat),
            Some(vec![0x80, b'A'])
        );
    }
}

/// Collect text writes for CONCAT-built String LOCALS (`let line = "== " +
/// name + " =="`): an initializer is a write in every sense except the
/// mutation arena's, and without one here no builder is ever planned, so the
/// local's frame slot silently stays zeroed (an empty string) at every later
/// read. Single-segment initializers (`let line = "Gate"`, `let line = other`)
/// stay on the frame-slot value-write path, which already materializes them --
/// only the Binary concat shape was unplanned.
fn collect_runtime_text_local_initializer_writes(
    state_storage: &StateStoragePlan,
    plan: &mut RuntimeTextPlan,
) {
    for (_, local) in state_storage.locals.iter() {
        if !local.initial_value.is_valid()
            || !matches!(
                state_storage.expressions.expression(local.initial_value),
                ExpressionNode::Binary(_)
            )
            || !is_obvious_runtime_text_value(&state_storage.expressions, local.initial_value)
        {
            continue;
        }

        // The write target is the local by name, matching how later reads
        // (guards, host-call arguments) spell it.
        let target = plan
            .expressions
            .insert_tree(&Expression::Name(NamePath::resolved(
                vec![local.name.clone()],
                local.symbol,
                local.symbol,
            )));
        let value = copy_text_value_folding_static_concat(
            &state_storage.expressions,
            local.initial_value,
            &mut plan.expressions,
        );

        plan.writes.insert(RuntimeTextWrite {
            source_key: local.source_key,
            statement_index: local.statement_index,
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
    }) || plan.uses.iter().any(|(_, text_use)| {
        text_use.source == crate::RuntimeTextSource::StoredPlace
            && expression_place_eq_across_tables(
                &plan.expressions,
                text_use.expression,
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
        | ExpressionNode::Atomic(_)
        | ExpressionNode::Binary(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::Call(_)
        | ExpressionNode::Cast(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Borrow(_)
        | ExpressionNode::Range(_)
        | ExpressionNode::StructLiteral(_)
        | ExpressionNode::Unary(_)
        | ExpressionNode::ZeroValue(_) => RuntimeTextBuilderSegmentKind::OtherExpression,
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
        ExpressionNode::Borrow(inner) => is_runtime_text_segment_like(table, inner.target),
        ExpressionNode::Unary(unary) => is_runtime_text_segment_like(table, unary.operand),
        ExpressionNode::Range(_) => false,
        ExpressionNode::Boolean(_)
        | ExpressionNode::Call(_)
        | ExpressionNode::Cast(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_) => true,
        ExpressionNode::ArrayLiteral(_)
        | ExpressionNode::Atomic(_)
        | ExpressionNode::StructLiteral(_)
        | ExpressionNode::ZeroValue(_) => false,
    }
}

fn contains_runtime_text_anchor(table: &ExpressionTable, expression: ExpressionHandle) -> bool {
    match table.expression(expression) {
        ExpressionNode::String(_) => true,
        ExpressionNode::Binary(binary) => {
            contains_runtime_text_anchor(table, binary.left)
                || contains_runtime_text_anchor(table, binary.right)
        }
        ExpressionNode::Borrow(inner) => contains_runtime_text_anchor(table, inner.target),
        ExpressionNode::Unary(unary) => contains_runtime_text_anchor(table, unary.operand),
        ExpressionNode::ArrayLiteral(_)
        | ExpressionNode::Atomic(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::Call(_)
        | ExpressionNode::Cast(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Indexed(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Member(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::Range(_)
        | ExpressionNode::StructLiteral(_)
        | ExpressionNode::ZeroValue(_) => false,
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
        | ExpressionNode::Atomic(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::Call(_)
        | ExpressionNode::Cast(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Borrow(_)
        | ExpressionNode::Range(_)
        | ExpressionNode::StructLiteral(_)
        | ExpressionNode::Unary(_)
        | ExpressionNode::ZeroValue(_) => RuntimeTextWriteKind::OtherExpression,
    }
}
