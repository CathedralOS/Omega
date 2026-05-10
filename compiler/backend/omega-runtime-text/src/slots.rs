use omega_core::arena::Arena;
use omega_core::symbols::SymbolHandle;
use omega_typed_program::expression::{
    ExpressionHandle, ExpressionNode, ExpressionTable, TableNamePath,
};
use omega_typed_program::name::ProgramName;

use super::places::expression_place_eq_in_table;
use super::{RuntimeTextPlan, RuntimeTextSlot, RuntimeTextSource};

pub(crate) fn build_runtime_text_slots(plan: &mut RuntimeTextPlan) -> Arena<RuntimeTextSlot> {
    let mut slots = Vec::new();
    let RuntimeTextPlan {
        expressions,
        buffers,
        uses,
        writes,
        ..
    } = plan;
    let buffer_places = buffers
        .iter()
        .filter_map(|(_, buffer)| {
            text_place_for_buffer_target(expressions, buffer.target).map(|place| BufferPlace {
                place,
                byte_capacity: buffer.byte_capacity,
            })
        })
        .collect::<Vec<_>>();

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

    for buffer_place in &buffer_places {
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

    let mut arena = Arena::new();
    arena.insert_many(slots);
    arena
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BufferPlace {
    place: ExpressionHandle,
    byte_capacity: usize,
}

fn push_or_update_text_slot(
    expressions: &ExpressionTable,
    slots: &mut Vec<RuntimeTextSlot>,
    place: ExpressionHandle,
    byte_capacity: usize,
    has_input_buffer: bool,
) {
    if let Some(existing_slot) = slots
        .iter_mut()
        .find(|slot| expression_place_eq_in_table(expressions, slot.place, place))
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

fn text_slot_capacity_for_use(
    expressions: &ExpressionTable,
    buffer_places: &[BufferPlace],
    expression: ExpressionHandle,
) -> usize {
    buffer_places
        .iter()
        .filter_map(|buffer_place| {
            expression_place_eq_in_table(expressions, buffer_place.place, expression)
                .then_some(buffer_place.byte_capacity)
        })
        .max()
        .unwrap_or(0)
}

fn text_place_has_input_buffer(
    expressions: &ExpressionTable,
    buffer_places: &[BufferPlace],
    expression: ExpressionHandle,
) -> bool {
    buffer_places.iter().any(|buffer_place| {
        expression_place_eq_in_table(expressions, buffer_place.place, expression)
    })
}

fn text_place_for_buffer_target(
    expressions: &mut ExpressionTable,
    target: ExpressionHandle,
) -> Option<ExpressionHandle> {
    match *expressions.expression(target) {
        ExpressionNode::Name(path) => {
            let members = expressions
                .copy_name_path_members_with_suffix(path.members, ProgramName::generated("text"));
            Some(expressions.insert(ExpressionNode::Name(TableNamePath {
                members,
                head_symbol: path.head_symbol,
                symbol: SymbolHandle::invalid(),
            })))
        }
        _ => None,
    }
}
