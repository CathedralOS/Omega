use crate::data::NativeDataObject;
use crate::host_calls::HostCall;
use crate::plan::NativePlan;
use crate::runtime_text::RuntimeTextSource;
use crate::runtime_text::places::expression_place_eq;
use omega_typed_program::expression::Expression;
use omega_typed_program::name::ProgramName;

pub(in crate::instructions::host_operations) fn find_runtime_text_input_buffer_data_object<
    'plan,
>(
    native_plan: &'plan NativePlan,
    host_call: &HostCall,
) -> Option<&'plan NativeDataObject> {
    let text_use = native_plan
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

    let text_slot = native_plan
        .runtime_text
        .slots
        .iter()
        .find(|(_, slot)| {
            expression_place_eq(&slot.place, &text_use.expression) && slot.has_input_buffer
        })
        .map(|(_, slot)| slot)?;

    let buffer = native_plan
        .runtime_text
        .buffers
        .iter()
        .find(|(_, buffer)| {
            text_place_for_buffer_target(&buffer.target)
                .is_some_and(|place| expression_place_eq(&place, &text_slot.place))
        })
        .map(|(_, buffer)| buffer)?;

    native_plan
        .data
        .objects
        .iter()
        .find(|(_, data_object)| {
            data_object.source_key == buffer.source_key
                && data_object.source_statement == buffer.statement_index
        })
        .map(|(_, data_object)| data_object)
}

pub(in crate::instructions) fn runtime_text_input_buffer_for_text_place<'plan>(
    native_plan: &'plan NativePlan,
    text_place: &Expression,
) -> Option<&'plan NativeDataObject> {
    let buffer = native_plan
        .runtime_text
        .buffers
        .iter()
        .find_map(|(_, buffer)| {
            text_place_for_buffer_target(&buffer.target)
                .is_some_and(|place| expression_place_eq(&place, text_place))
                .then_some(buffer)
        })?;

    native_plan
        .data
        .objects
        .iter()
        .find(|(_, data_object)| {
            data_object.source_key == buffer.source_key
                && data_object.source_statement == buffer.statement_index
        })
        .map(|(_, data_object)| data_object)
}

pub(super) fn text_place_for_buffer_target(target: &Expression) -> Option<Expression> {
    text_expression_for_buffer_target(target)
}

pub(super) fn text_expression_for_buffer_target(target: &Expression) -> Option<Expression> {
    match target {
        Expression::Name(path) => {
            let mut text_path = path.clone();
            text_path.push(ProgramName::generated("text"));
            Some(Expression::Name(text_path))
        }
        _ => None,
    }
}
