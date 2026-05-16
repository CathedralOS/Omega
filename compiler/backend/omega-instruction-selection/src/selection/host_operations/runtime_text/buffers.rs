use crate::InstructionSelectionInput;
use omega_checked_trees::expression::Expression;
use omega_platform_interface::HostCall;
use omega_runtime_text::RuntimeTextSource;
use omega_runtime_text::places::{
    expression_name_with_suffix_eq_in_table, expression_name_with_suffix_eq_tree,
    expression_place_eq, expression_place_eq_in_table,
};
use omega_target_operations::{TargetDataObject, TargetDataObjectHandle};

pub(in crate::selection::host_operations) fn find_runtime_text_input_buffer_data<'plan>(
    input: &'plan InstructionSelectionInput<'plan>,
    host_call: &HostCall,
) -> Option<(TargetDataObjectHandle, &'plan TargetDataObject)> {
    let text_use = input
        .runtime_text
        .uses
        .iter()
        .find(|(_, text_use)| {
            text_use.source_key == host_call.source_key
                && text_use.statement_index == host_call.statement_index
                && text_use.platform_call == host_call.platform_call
                && text_use.source == RuntimeTextSource::StoredPlace
        })
        .map(|(_, text_use)| text_use)?;

    let text_slot = input
        .runtime_text
        .slots
        .iter()
        .find(|(_, slot)| {
            expression_place_eq_in_table(
                &input.runtime_text.expressions,
                slot.place,
                text_use.expression,
            ) && slot.has_input_buffer
        })
        .map(|(_, slot)| slot)?;

    let buffer = input
        .runtime_text
        .buffers
        .iter()
        .find(|(_, buffer)| {
            expression_name_with_suffix_eq_in_table(
                &input.runtime_text.expressions,
                buffer.target,
                text_slot.place,
                "text",
            )
        })
        .map(|(_, buffer)| buffer)?;

    input.data.objects.iter().find(|(_, data_object)| {
        data_object.source_key == buffer.source_key
            && data_object.source_statement == buffer.statement_index
    })
}

pub(in crate::selection::host_operations) fn find_runtime_text_input_buffer_data_object<'plan>(
    input: &'plan InstructionSelectionInput<'plan>,
    host_call: &HostCall,
) -> Option<&'plan TargetDataObject> {
    find_runtime_text_input_buffer_data(input, host_call).map(|(_, data_object)| data_object)
}

pub(in crate::selection) fn runtime_text_input_buffer_data_for_text_place<'plan>(
    input: &'plan InstructionSelectionInput<'plan>,
    text_place: &Expression,
) -> Option<(TargetDataObjectHandle, &'plan TargetDataObject)> {
    let buffer = input.runtime_text.buffers.iter().find_map(|(_, buffer)| {
        (expression_name_with_suffix_eq_tree(
            &input.runtime_text.expressions,
            buffer.target,
            text_place,
            "text",
        ) || expression_place_eq(
            &input.runtime_text.expressions.to_tree(buffer.target),
            text_place,
        ))
        .then_some(buffer)
    })?;

    input.data.objects.iter().find(|(_, data_object)| {
        data_object.source_key == buffer.source_key
            && data_object.source_statement == buffer.statement_index
    })
}
