use psi_arena::Arena;
use psi_checked_trees::expression::{ExpressionHandle, ExpressionNode, ExpressionTable};

use super::places::expression_place_eq_in_table;
use super::{RuntimeTextPlan, RuntimeTextSlot, RuntimeTextSource};

pub(crate) fn build_runtime_text_slots(plan: &mut RuntimeTextPlan) -> Arena<RuntimeTextSlot> {
    let RuntimeTextPlan {
        expressions,
        buffers,
        uses,
        writes,
        ..
    } = plan;
    let slot_capacity = uses
        .len()
        .checked_add(buffers.len())
        .and_then(|count| count.checked_add(writes.len()))
        .expect("runtime text slot capacity overflow");
    let mut slots = Arena::with_capacity(slot_capacity);
    let mut buffer_places = Arena::with_capacity(buffers.len());
    for (_, buffer) in buffers.iter() {
        if buffer.text_place.is_valid() {
            buffer_places.insert(BufferPlace {
                place: buffer.text_place,
                byte_capacity: buffer.byte_capacity,
            });
        }
    }

    for (_, text_use) in uses.iter() {
        if text_use.source != RuntimeTextSource::StoredPlace {
            continue;
        }

        let byte_capacity =
            text_slot_capacity_for_use(expressions, &buffer_places, text_use.expression);
        let has_input_buffer =
            text_place_has_input_buffer(expressions, &buffer_places, text_use.expression);
        push_or_update_text_slot(
            expressions,
            &mut slots,
            text_use.expression,
            byte_capacity,
            has_input_buffer,
        );
    }

    for (_, buffer_place) in buffer_places.iter() {
        push_or_update_text_slot(
            expressions,
            &mut slots,
            buffer_place.place,
            buffer_place.byte_capacity,
            true,
        );
    }

    for (_, write) in writes.iter() {
        push_or_update_text_slot(expressions, &mut slots, write.target, 0, false);
    }

    slots
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct BufferPlace {
    place: ExpressionHandle,
    byte_capacity: usize,
}

fn push_or_update_text_slot(
    expressions: &ExpressionTable,
    slots: &mut Arena<RuntimeTextSlot>,
    place: ExpressionHandle,
    byte_capacity: usize,
    has_input_buffer: bool,
) {
    if let Some((slot_handle, _)) = slots
        .iter()
        .find(|(_, slot)| expression_place_eq_in_table(expressions, slot.place, place))
    {
        let existing_slot = slots.get_mut(slot_handle);
        existing_slot.byte_capacity = existing_slot.byte_capacity.max(byte_capacity);
        existing_slot.has_input_buffer |= has_input_buffer;
        return;
    }

    slots.insert(RuntimeTextSlot {
        place,
        byte_capacity,
        has_input_buffer,
    });
}

fn text_slot_capacity_for_use(
    expressions: &ExpressionTable,
    buffer_places: &Arena<BufferPlace>,
    expression: ExpressionHandle,
) -> usize {
    buffer_places
        .iter()
        .filter_map(|(_, buffer_place)| {
            expression_place_eq_in_table(expressions, buffer_place.place, expression)
                .then_some(buffer_place.byte_capacity)
        })
        .max()
        .unwrap_or(0)
}

fn text_place_has_input_buffer(
    expressions: &ExpressionTable,
    buffer_places: &Arena<BufferPlace>,
    expression: ExpressionHandle,
) -> bool {
    buffer_places.iter().any(|(_, buffer_place)| {
        expression_place_eq_in_table(expressions, buffer_place.place, expression)
    })
}

pub(crate) fn text_place_for_buffer_target(
    expressions: &mut ExpressionTable,
    target: ExpressionHandle,
) -> ExpressionHandle {
    if expressions.expression_is_stored_place(target) {
        return target;
    }

    match *expressions.expression(target) {
        ExpressionNode::Mutable(inner) => text_place_for_buffer_target(expressions, inner),
        _ => ExpressionHandle::invalid(),
    }
}
