use crate::InstructionSelectionInput;
use omega_abstract_operations::{AbstractDataObject, AbstractDataObjectHandle};
use omega_platform_interface::HostCall;
use omega_runtime_text::RuntimeTextSource;
use omega_runtime_text::places::{
    expression_place_eq_across_tables, expression_place_eq_in_table, expression_place_eq_table_tree,
};
use psi_checked_trees::expression::{Expression, ExpressionHandle, ExpressionTable};

pub(in crate::selection::host_operations) fn find_runtime_text_input_buffer_data(
    input: &InstructionSelectionInput<'_>,
    host_call: &HostCall,
) -> AbstractDataObjectHandle {
    let text_use = input
        .runtime_text
        .uses
        .iter()
        .find(|(_, text_use)| {
            text_use.source_key == host_call.source_key
                && text_use.statement_index == host_call.statement_index
                && text_use.source == RuntimeTextSource::StoredPlace
        })
        .map(|(_, text_use)| text_use);
    let Some(text_use) = text_use else {
        return AbstractDataObjectHandle::invalid();
    };

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
        .map(|(_, slot)| slot);
    let Some(text_slot) = text_slot else {
        return AbstractDataObjectHandle::invalid();
    };

    let buffer = input
        .runtime_text
        .buffers
        .iter()
        .find(|(_, buffer)| {
            expression_place_eq_in_table(
                &input.runtime_text.expressions,
                buffer.text_place,
                text_slot.place,
            )
        })
        .map(|(_, buffer)| buffer);
    let Some(buffer) = buffer else {
        return AbstractDataObjectHandle::invalid();
    };

    input
        .data
        .objects
        .iter()
        .find(|(_, data_object)| {
            data_object.source_key == buffer.source_key
                && data_object.source_statement == buffer.statement_index
        })
        .map(|(handle, _)| handle)
        .unwrap_or_else(AbstractDataObjectHandle::invalid)
}

pub(in crate::selection::host_operations) fn find_runtime_text_input_buffer_data_object<'plan>(
    input: &'plan InstructionSelectionInput<'plan>,
    host_call: &HostCall,
) -> Option<&'plan AbstractDataObject> {
    let handle = find_runtime_text_input_buffer_data(input, host_call);
    handle.is_valid().then(|| input.data.objects.get(handle))
}

pub(in crate::selection) fn runtime_text_input_buffer_data_for_text_place(
    input: &InstructionSelectionInput<'_>,
    text_place: &Expression,
) -> AbstractDataObjectHandle {
    let buffer = input.runtime_text.buffers.iter().find_map(|(_, buffer)| {
        (expression_place_eq_table_tree(
            &input.runtime_text.expressions,
            buffer.text_place,
            text_place,
        ) || expression_place_eq_table_tree(
            &input.runtime_text.expressions,
            buffer.target,
            text_place,
        ))
        .then_some(buffer)
    });
    let Some(buffer) = buffer else {
        return AbstractDataObjectHandle::invalid();
    };

    input
        .data
        .objects
        .iter()
        .find(|(_, data_object)| {
            data_object.source_key == buffer.source_key
                && data_object.source_statement == buffer.statement_index
        })
        .map(|(handle, _)| handle)
        .unwrap_or_else(AbstractDataObjectHandle::invalid)
}

pub(in crate::selection) fn runtime_text_input_buffer_data_for_text_place_in_table(
    input: &InstructionSelectionInput<'_>,
    text_place_expressions: &ExpressionTable,
    text_place: ExpressionHandle,
) -> AbstractDataObjectHandle {
    let buffer = input.runtime_text.buffers.iter().find_map(|(_, buffer)| {
        (expression_place_eq_across_tables(
            &input.runtime_text.expressions,
            buffer.text_place,
            text_place_expressions,
            text_place,
        ) || expression_place_eq_across_tables(
            &input.runtime_text.expressions,
            buffer.target,
            text_place_expressions,
            text_place,
        ))
        .then_some(buffer)
    });
    let Some(buffer) = buffer else {
        return AbstractDataObjectHandle::invalid();
    };

    input
        .data
        .objects
        .iter()
        .find(|(_, data_object)| {
            data_object.source_key == buffer.source_key
                && data_object.source_statement == buffer.statement_index
        })
        .map(|(handle, _)| handle)
        .unwrap_or_else(AbstractDataObjectHandle::invalid)
}
