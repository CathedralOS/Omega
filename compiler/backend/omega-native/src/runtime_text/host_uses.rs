use crate::abi::PlatformCallData;
use crate::host_calls::{HostCall, HostCallArgumentKind, HostCallPlan};
use omega_typed_program::expression::Expression;

use super::{
    RuntimeTextBuffer, RuntimeTextPlan, RuntimeTextSource, RuntimeTextUse,
};

pub(in crate::runtime_text) fn collect_host_call_runtime_text(
    host_calls: &HostCallPlan,
    host_call: &HostCall,
    plan: &mut RuntimeTextPlan,
) {
    match host_call.data {
        PlatformCallData::FirstTextArgument { append_newline } => {
            collect_runtime_text_use(host_calls, host_call, plan, append_newline);
        }
        PlatformCallData::MutableOutputBuffer { byte_capacity } => {
            collect_runtime_text_buffer(host_calls, host_call, plan, byte_capacity);
        }
        PlatformCallData::None => {}
    }
}

fn collect_runtime_text_use(
    host_calls: &HostCallPlan,
    host_call: &HostCall,
    plan: &mut RuntimeTextPlan,
    append_newline: bool,
) {
    let Some(first_argument) = first_host_argument(host_calls, host_call) else {
        return;
    };

    if let HostCallArgumentKind::Expression(expression) = &first_argument.kind {
        plan.uses.insert(RuntimeTextUse {
            source_key: host_call.source_key,
            machine: host_call.machine.clone(),
            state: host_call.state.clone(),
            statement_index: host_call.statement_index,
            platform_call: host_call.platform_call.clone(),
            expression: expression.clone(),
            source: classify_runtime_text_source(expression),
            append_newline,
        });
    }
}

fn collect_runtime_text_buffer(
    host_calls: &HostCallPlan,
    host_call: &HostCall,
    plan: &mut RuntimeTextPlan,
    byte_capacity: usize,
) {
    let Some(first_argument) = first_host_argument(host_calls, host_call) else {
        return;
    };

    let HostCallArgumentKind::Expression(Expression::Mutable(target)) = &first_argument.kind else {
        return;
    };

    plan.buffers.insert(RuntimeTextBuffer {
        source_key: host_call.source_key,
        machine: host_call.machine.clone(),
        state: host_call.state.clone(),
        statement_index: host_call.statement_index,
        platform_call: host_call.platform_call.clone(),
        target: (**target).clone(),
        byte_capacity,
    });
}

fn first_host_argument<'plan>(
    host_calls: &'plan HostCallPlan,
    host_call: &HostCall,
) -> Option<&'plan crate::host_calls::HostCallArgument> {
    host_calls
        .arguments
        .span(host_call.arguments)
        .and_then(|arguments| arguments.first())
}

fn classify_runtime_text_source(expression: &Expression) -> RuntimeTextSource {
    match expression {
        Expression::Name(_) | Expression::Indexed(_) => RuntimeTextSource::StoredPlace,
        Expression::Binary(_) => RuntimeTextSource::GeneratedString,
        Expression::Mutable(_) => RuntimeTextSource::MutablePlace,
        Expression::ArrayLiteral(_)
        | Expression::Boolean(_)
        | Expression::Float(_)
        | Expression::Integer(_)
        | Expression::StructLiteral(_)
        | Expression::String(_) => RuntimeTextSource::OtherExpression,
    }
}
