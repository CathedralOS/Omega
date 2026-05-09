use omega_core::arena::Arena;
use omega_typed_program::expression::Expression;
use omega_typed_program::name::ProgramName;

use super::places::expression_place_eq;
use super::{RuntimeTextPlan, RuntimeTextSlot, RuntimeTextSource};

pub(in crate::runtime_text) fn build_runtime_text_slots(
    plan: &RuntimeTextPlan,
) -> Arena<RuntimeTextSlot> {
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
    if let Some(existing_slot) = slots
        .iter_mut()
        .find(|slot| expression_place_eq(&slot.place, &place))
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
                .is_some_and(|place| expression_place_eq(&place, expression))
                .then_some(buffer.byte_capacity)
        })
        .max()
        .unwrap_or(0)
}

fn text_place_has_input_buffer(plan: &RuntimeTextPlan, expression: &Expression) -> bool {
    plan.buffers.iter().any(|(_, buffer)| {
        text_place_for_buffer_target(&buffer.target)
            .is_some_and(|place| expression_place_eq(&place, expression))
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
