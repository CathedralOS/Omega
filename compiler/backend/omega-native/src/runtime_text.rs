mod host_uses;
mod model;

use crate::plan::NativePlan;
use omega_core::arena::Arena;
use omega_typed_program::expression::{BinaryOperator, Expression};
use omega_typed_program::name::ProgramName;
use host_uses::collect_host_call_runtime_text;
pub use model::{
    RuntimeTextBuffer, RuntimeTextBuilder, RuntimeTextBuilderSegment,
    RuntimeTextBuilderSegmentKind, RuntimeTextPlan, RuntimeTextSlot, RuntimeTextSource,
    RuntimeTextUse, RuntimeTextWrite, RuntimeTextWriteKind,
};

pub fn build_runtime_text_plan(native_plan: &NativePlan) -> RuntimeTextPlan {
    let mut plan = RuntimeTextPlan::default();

    for (_, host_call) in native_plan.host_calls.calls.iter() {
        collect_host_call_runtime_text(&native_plan.host_calls, host_call, &mut plan);
    }
    collect_runtime_text_writes(native_plan, &mut plan);
    collect_runtime_text_builders(&mut plan);
    plan.slots = build_runtime_text_slots(&plan);

    plan
}

fn build_runtime_text_slots(plan: &RuntimeTextPlan) -> Arena<RuntimeTextSlot> {
    let mut slots = Vec::new();

    for (_, text_use) in plan.uses.iter() {
        if text_use.source != RuntimeTextSource::StoredPlace {
            continue;
        }

        push_or_update_text_slot(
            &mut slots,
            text_use.expression.clone(),
            text_slot_capacity_for_use(plan, &text_use.expression),
            text_place_has_input_buffer(plan, &text_use.expression),
        );
    }

    for (_, buffer) in plan.buffers.iter() {
        if let Some(place) = text_place_for_buffer_target(&buffer.target) {
            push_or_update_text_slot(&mut slots, place, buffer.byte_capacity, true);
        }
    }

    for (_, write) in plan.writes.iter() {
        push_or_update_text_slot(&mut slots, write.target.clone(), 0, false);
    }

    let mut arena = Arena::new();
    arena.insert_many(slots);
    arena
}

fn push_or_update_text_slot(
    slots: &mut Vec<RuntimeTextSlot>,
    place: Expression,
    byte_capacity: usize,
    has_input_buffer: bool,
) {
    let place_name = place.display_name();
    if let Some(existing_slot) = slots
        .iter_mut()
        .find(|slot| slot.place.display_name() == place_name)
    {
        existing_slot.byte_capacity = existing_slot.byte_capacity.max(byte_capacity);
        existing_slot.has_input_buffer |= has_input_buffer;
        return;
    }

    slots.push(RuntimeTextSlot {
        place,
        byte_capacity,
        has_input_buffer,
    });
}

fn text_slot_capacity_for_use(plan: &RuntimeTextPlan, expression: &Expression) -> usize {
    plan.buffers
        .iter()
        .filter_map(|(_, buffer)| {
            text_place_for_buffer_target(&buffer.target)
                .is_some_and(|place| place.display_name() == expression.display_name())
                .then_some(buffer.byte_capacity)
        })
        .max()
        .unwrap_or(0)
}

fn text_place_has_input_buffer(plan: &RuntimeTextPlan, expression: &Expression) -> bool {
    plan.buffers.iter().any(|(_, buffer)| {
        text_place_for_buffer_target(&buffer.target)
            .is_some_and(|place| place.display_name() == expression.display_name())
    })
}

fn text_place_for_buffer_target(target: &Expression) -> Option<Expression> {
    match target {
        Expression::Name(path) => {
            let mut text_path = path.clone();
            text_path.push(ProgramName::generated("text"));
            Some(Expression::Name(text_path))
        }
        _ => None,
    }
}

fn collect_runtime_text_writes(native_plan: &NativePlan, plan: &mut RuntimeTextPlan) {
    for (_, mutation) in native_plan.state_storage.mutations.iter() {
        if !is_text_place(&mutation.target) {
            continue;
        }

        plan.writes.insert(RuntimeTextWrite {
            source_key: mutation.source_key,
            machine: mutation.machine.clone(),
            state: mutation.state.clone(),
            statement_index: mutation.statement_index,
            target: mutation.target.clone(),
            value: mutation.value.clone(),
            kind: classify_runtime_text_write(&mutation.value),
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
        collect_builder_segments(&write.value, &mut segments);
        let segment_span = plan.builder_segments.insert_many(segments);
        plan.builders.insert(RuntimeTextBuilder {
            source_key: write.source_key,
            machine: write.machine,
            state: write.state,
            statement_index: write.statement_index,
            target: write.target,
            segments: segment_span,
        });
    }
}

fn collect_builder_segments(
    expression: &Expression,
    segments: &mut Vec<RuntimeTextBuilderSegment>,
) {
    if let Expression::Binary(binary) = expression
        && binary.operator == BinaryOperator::Add
    {
        collect_builder_segments(&binary.left, segments);
        collect_builder_segments(&binary.right, segments);
        return;
    }

    segments.push(RuntimeTextBuilderSegment {
        expression: expression.clone(),
        kind: classify_runtime_text_builder_segment(expression),
    });
}

fn classify_runtime_text_builder_segment(expression: &Expression) -> RuntimeTextBuilderSegmentKind {
    match expression {
        Expression::String(_) => RuntimeTextBuilderSegmentKind::StaticText,
        Expression::Name(_) | Expression::Indexed(_) => RuntimeTextBuilderSegmentKind::StoredPlace,
        Expression::ArrayLiteral(_)
        | Expression::Binary(_)
        | Expression::Boolean(_)
        | Expression::Float(_)
        | Expression::Integer(_)
        | Expression::Mutable(_)
        | Expression::StructLiteral(_) => RuntimeTextBuilderSegmentKind::OtherExpression,
    }
}

fn is_text_place(expression: &Expression) -> bool {
    match expression {
        Expression::Name(path) => path.last().is_some_and(|segment| segment == "text"),
        Expression::Indexed(indexed) => is_text_place(&indexed.collection),
        Expression::Mutable(expression) => is_text_place(expression),
        _ => false,
    }
}

fn classify_runtime_text_write(expression: &Expression) -> RuntimeTextWriteKind {
    match expression {
        Expression::String(_) => RuntimeTextWriteKind::StaticText,
        Expression::Name(_) | Expression::Indexed(_) => RuntimeTextWriteKind::StoredCopy,
        Expression::Binary(_) => RuntimeTextWriteKind::GeneratedString,
        Expression::ArrayLiteral(_)
        | Expression::Boolean(_)
        | Expression::Float(_)
        | Expression::Integer(_)
        | Expression::Mutable(_)
        | Expression::StructLiteral(_) => RuntimeTextWriteKind::OtherExpression,
    }
}
