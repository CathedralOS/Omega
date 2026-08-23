use omega_calling_conventions::PlatformCallData;
use omega_control_flow::StateKey;
use omega_platform_interface::{HostCall, HostCallArgumentKind, HostCallPlan};
use omega_runtime_text::RuntimeTextPlan;
use omega_target_operations::{TargetDataObject, TargetDataObjectKind, TargetDataPlan};

pub(super) fn collect_host_call_data(
    host_calls: &HostCallPlan,
    host_call: &HostCall,
    data_plan: &mut TargetDataPlan,
) {
    match host_call.data {
        PlatformCallData::FirstTextArgument { append_newline } => {
            collect_text_argument_data(host_calls, host_call, data_plan, append_newline);
        }
        PlatformCallData::MutableOutputBuffer { .. } => {}
        PlatformCallData::SingleByteWrite => {
            collect_byte_literal_data(host_calls, host_call, data_plan);
        }
        // A constant result contributes no data objects; the byte read
        // writes straight into the ByteRead slot (no scratch object).
        PlatformCallData::None
        | PlatformCallData::SingleByteRead
        | PlatformCallData::ConstantResult { .. }
        | PlatformCallData::ConstantArgument { .. }
        | PlatformCallData::ConstantArguments { .. }
        | PlatformCallData::DirectoryRelativePathPair { .. }
        | PlatformCallData::OmitTrailingArgument
        | PlatformCallData::TimespecResult { .. }
        | PlatformCallData::TimespecArgument => {}
    }
}

/// A `write_byte(<integer literal>)` argument has no storage place to write
/// from, so its low byte is staged as a 1-byte data object (the
/// string-literal precedent); place-backed arguments contribute nothing.
fn collect_byte_literal_data(
    host_calls: &HostCallPlan,
    host_call: &HostCall,
    data_plan: &mut TargetDataPlan,
) {
    let Some(arguments) = host_calls.arguments.span(host_call.arguments) else {
        return;
    };
    let Some(first_argument) = arguments.first() else {
        return;
    };
    // Integer literals arrive PRE-RESOLVED as the Integer kind (host-call
    // argument lowering folds `ExpressionNode::Integer` before the plan is
    // built), so that is the shape staged here; place-backed arguments stay
    // Expression and contribute nothing.
    let HostCallArgumentKind::Integer(value) = first_argument.kind else {
        return;
    };

    let offset = data_plan.bytes.len();
    let byte_span = data_plan.bytes.insert_many(std::iter::once(value as u8));
    let symbol_index = data_plan.objects.len() + 1;

    data_plan.objects.insert(TargetDataObject {
        symbol: format!("omega_byte_literal_{symbol_index}").into(),
        kind: TargetDataObjectKind::StaticString,
        offset,
        bytes: byte_span,
        alignment: 1,
        source_key: host_call.source_key,
        source_statement: host_call.statement_index,
    });
}

pub(super) fn collect_runtime_text_buffer_data(
    runtime_text: &RuntimeTextPlan,
    data_plan: &mut TargetDataPlan,
) {
    for (_, buffer) in runtime_text.buffers.iter() {
        collect_runtime_text_buffer(
            buffer.source_key,
            buffer.statement_index,
            data_plan,
            buffer.byte_capacity,
        );
    }
}

pub(super) fn collect_newline_data(host_calls: &HostCallPlan, data_plan: &mut TargetDataPlan) {
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
    let byte_span = data_plan.bytes.insert_many(std::iter::once(b'\n'));

    data_plan.objects.insert(TargetDataObject {
        symbol: "omega_newline".into(),
        kind: TargetDataObjectKind::HostNewline,
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
    data_plan: &mut TargetDataPlan,
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

    let offset = data_plan.bytes.len();
    let byte_span = data_plan
        .bytes
        .insert_many(text.iter().copied().chain(append_newline.then_some(b'\n')));
    let symbol_index = data_plan.objects.len() + 1;

    data_plan.objects.insert(TargetDataObject {
        symbol: format!("omega_string_literal_{symbol_index}").into(),
        kind: TargetDataObjectKind::StaticString,
        offset,
        bytes: byte_span,
        alignment: 1,
        source_key: host_call.source_key,
        source_statement: host_call.statement_index,
    });
}

fn collect_runtime_text_buffer(
    source_key: StateKey,
    source_statement: usize,
    data_plan: &mut TargetDataPlan,
    byte_capacity: usize,
) {
    let offset = data_plan.bytes.len();
    let byte_span = data_plan
        .bytes
        .insert_many(std::iter::repeat(0).take(byte_capacity));
    let symbol_index = data_plan.objects.len() + 1;

    data_plan.objects.insert(TargetDataObject {
        symbol: format!("omega_mut_buffer_{symbol_index}").into(),
        kind: TargetDataObjectKind::RuntimeTextBuffer,
        offset,
        bytes: byte_span,
        alignment: 16,
        source_key,
        source_statement,
    });
}
