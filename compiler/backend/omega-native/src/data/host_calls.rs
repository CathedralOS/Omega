use crate::abi::PlatformCallData;
use crate::data::{NativeDataObject, NativeDataPlan};
use crate::host_calls::{HostCall, HostCallArgumentKind, HostCallPlan};
use omega_control_flow::StateKey;

pub(super) fn collect_host_call_data(
    host_calls: &HostCallPlan,
    host_call: &HostCall,
    data_plan: &mut NativeDataPlan,
) {
    match host_call.data {
        PlatformCallData::FirstTextArgument { append_newline } => {
            collect_text_argument_data(host_calls, host_call, data_plan, append_newline);
        }
        PlatformCallData::MutableOutputBuffer { byte_capacity } => {
            collect_mutable_output_buffer(host_call, data_plan, byte_capacity);
        }
        PlatformCallData::None => {}
    }
}

pub(super) fn collect_newline_data(host_calls: &HostCallPlan, data_plan: &mut NativeDataPlan) {
    let needs_newline = host_calls.calls.iter().any(|(_, host_call)| {
        if !matches!(
            host_call.data,
            PlatformCallData::FirstTextArgument {
                append_newline: true
            }
        ) {
            return false;
        }
        host_calls
            .arguments
            .span(host_call.arguments)
            .and_then(|arguments| arguments.first())
            .is_some_and(|argument| matches!(argument.kind, HostCallArgumentKind::Expression(_)))
    });
    if !needs_newline {
        return;
    }

    let offset = data_plan.bytes.len();
    let byte_span = data_plan.bytes.insert_many(vec![b'\n']);

    data_plan.objects.insert(NativeDataObject {
        symbol: "omega_newline".to_owned(),
        offset,
        bytes: byte_span,
        alignment: 1,
        source_key: StateKey::default(),
        source_statement: usize::MAX,
    });
}

fn collect_text_argument_data(
    host_calls: &HostCallPlan,
    host_call: &HostCall,
    data_plan: &mut NativeDataPlan,
    append_newline: bool,
) {
    let Some(arguments) = host_calls.arguments.span(host_call.arguments) else {
        return;
    };

    let Some(first_argument) = arguments.first() else {
        return;
    };

    let HostCallArgumentKind::Text(text) = &first_argument.kind else {
        return;
    };

    let mut bytes = text.as_bytes().to_vec();
    if append_newline {
        bytes.push(b'\n');
    }

    let offset = data_plan.bytes.len();
    let byte_span = data_plan.bytes.insert_many(bytes);
    let symbol_index = data_plan.objects.len() + 1;

    data_plan.objects.insert(NativeDataObject {
        symbol: format!("omega_string_literal_{symbol_index}"),
        offset,
        bytes: byte_span,
        alignment: 1,
        source_key: host_call.source_key,
        source_statement: host_call.statement_index,
    });
}

fn collect_mutable_output_buffer(
    host_call: &HostCall,
    data_plan: &mut NativeDataPlan,
    byte_capacity: usize,
) {
    let offset = data_plan.bytes.len();
    let byte_span = data_plan.bytes.insert_many(vec![0; byte_capacity]);
    let symbol_index = data_plan.objects.len() + 1;

    data_plan.objects.insert(NativeDataObject {
        symbol: format!("omega_mut_buffer_{symbol_index}"),
        offset,
        bytes: byte_span,
        alignment: 16,
        source_key: host_call.source_key,
        source_statement: host_call.statement_index,
    });
}
