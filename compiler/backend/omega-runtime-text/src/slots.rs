use omega_core::arena::Arena;
use omega_core::symbols::SymbolHandle;
use omega_typed_program::expression::{ExpressionHandle, ExpressionNode, TableNamePath};
use omega_typed_program::name::ProgramName;

use super::places::expression_place_eq_in_table;
use super::{RuntimeTextPlan, RuntimeTextSlot, RuntimeTextSource};

pub(crate) fn build_runtime_text_slots(plan: &mut RuntimeTextPlan) -> Arena<RuntimeTextSlot> {
    let mut slots = Vec::new();

    let uses = plan
        .uses
        .iter()
        .map(|(_, text_use)| text_use.clone())
        .collect::<Vec<_>>();
    for text_use in uses {
        if text_use.source != RuntimeTextSource::StoredPlace {
            continue;
        }

        let byte_capacity = text_slot_capacity_for_use(plan, text_use.expression);
        let has_input_buffer = text_place_has_input_buffer(plan, text_use.expression);
        push_or_update_text_slot(
            plan,
            &mut slots,
            text_use.expression,
            byte_capacity,
            has_input_buffer,
        );
    }

    let buffers = plan
        .buffers
        .iter()
        .map(|(_, buffer)| buffer.clone())
        .collect::<Vec<_>>();
    for buffer in buffers {
        if let Some(place) = text_place_for_buffer_target(plan, buffer.target) {
            push_or_update_text_slot(plan, &mut slots, place, buffer.byte_capacity, true);
        }
    }

    let writes = plan
        .writes
        .iter()
        .map(|(_, write)| write.clone())
        .collect::<Vec<_>>();
    for write in writes {
        push_or_update_text_slot(plan, &mut slots, write.target, 0, false);
    }

    let mut arena = Arena::new();
    arena.insert_many(slots);
    arena
}

fn push_or_update_text_slot(
    plan: &RuntimeTextPlan,
    slots: &mut Vec<RuntimeTextSlot>,
    place: ExpressionHandle,
    byte_capacity: usize,
    has_input_buffer: bool,
) {
    if let Some(existing_slot) = slots
        .iter_mut()
        .find(|slot| expression_place_eq_in_table(&plan.expressions, slot.place, place))
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

fn text_slot_capacity_for_use(plan: &mut RuntimeTextPlan, expression: ExpressionHandle) -> usize {
    let buffers = plan
        .buffers
        .iter()
        .map(|(_, buffer)| buffer.clone())
        .collect::<Vec<_>>();
    buffers
        .iter()
        .filter_map(|buffer| {
            text_place_for_buffer_target(plan, buffer.target)
                .is_some_and(|place| {
                    expression_place_eq_in_table(&plan.expressions, place, expression)
                })
                .then_some(buffer.byte_capacity)
        })
        .max()
        .unwrap_or(0)
}

fn text_place_has_input_buffer(plan: &mut RuntimeTextPlan, expression: ExpressionHandle) -> bool {
    let buffers = plan
        .buffers
        .iter()
        .map(|(_, buffer)| buffer.clone())
        .collect::<Vec<_>>();
    buffers.iter().any(|buffer| {
        text_place_for_buffer_target(plan, buffer.target)
            .is_some_and(|place| expression_place_eq_in_table(&plan.expressions, place, expression))
    })
}

fn text_place_for_buffer_target(
    plan: &mut RuntimeTextPlan,
    target: ExpressionHandle,
) -> Option<ExpressionHandle> {
    match *plan.expressions.expression(target) {
        ExpressionNode::Name(path) => {
            let members = plan
                .expressions
                .copy_name_path_members_with_suffix(path.members, ProgramName::generated("text"));
            Some(plan.expressions.insert(ExpressionNode::Name(TableNamePath {
                members,
                head_symbol: path.head_symbol,
                symbol: SymbolHandle::invalid(),
            })))
        }
        _ => None,
    }
}
