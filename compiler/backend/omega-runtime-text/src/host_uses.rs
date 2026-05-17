use omega_calling_conventions::PlatformCallData;
use omega_checked_trees::expression::{ExpressionHandle, ExpressionNode};
use omega_platform_interface::{HostCall, HostCallArgumentKind, HostCallPlan};

use super::places::expression_place_eq_across_tables;
use super::slots::text_place_for_buffer_target;
use super::{RuntimeTextBuffer, RuntimeTextPlan, RuntimeTextSource, RuntimeTextUse};

const DEFAULT_RUNTIME_TEXT_OUTPUT_BUFFER_CAPACITY: usize = 256;

pub(crate) fn collect_host_call_runtime_text(
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
        let expression_handle = plan
            .expressions
            .copy_from(&host_calls.expressions, *expression);
        let source = classify_runtime_text_source(host_calls, *expression);
        plan.uses.insert(RuntimeTextUse {
            source_key: host_call.source_key,
            statement_index: host_call.statement_index,
            expression: expression_handle,
            source,
            append_newline,
        });

        if source == RuntimeTextSource::StoredPlace
            && let Some(target) =
                output_buffer_target_for_text_expression(host_calls, plan, *expression)
        {
            plan.buffers.insert(RuntimeTextBuffer {
                source_key: host_call.source_key,
                statement_index: host_call.statement_index,
                target,
                text_place: expression_handle,
                byte_capacity: DEFAULT_RUNTIME_TEXT_OUTPUT_BUFFER_CAPACITY,
            });
        }
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

    let HostCallArgumentKind::Expression(expression) = &first_argument.kind else {
        return;
    };
    let ExpressionNode::Mutable(target) = host_calls.expressions.expression(*expression) else {
        return;
    };

    let target = plan.expressions.copy_from(&host_calls.expressions, *target);
    let text_place = text_place_for_buffer_target(&mut plan.expressions, target)
        .unwrap_or(ExpressionHandle::invalid());

    plan.buffers.insert(RuntimeTextBuffer {
        source_key: host_call.source_key,
        statement_index: host_call.statement_index,
        target,
        text_place,
        byte_capacity,
    });
}

fn first_host_argument<'plan>(
    host_calls: &'plan HostCallPlan,
    host_call: &HostCall,
) -> Option<&'plan omega_platform_interface::HostCallArgument> {
    host_calls
        .arguments
        .span(host_call.arguments)
        .and_then(|arguments| arguments.first())
}

fn classify_runtime_text_source(
    host_calls: &HostCallPlan,
    expression: ExpressionHandle,
) -> RuntimeTextSource {
    match host_calls.expressions.expression(expression) {
        ExpressionNode::Name(_) | ExpressionNode::Indexed(_) | ExpressionNode::Member(_) => {
            RuntimeTextSource::StoredPlace
        }
        ExpressionNode::Binary(_) => RuntimeTextSource::GeneratedString,
        ExpressionNode::Mutable(_) => RuntimeTextSource::MutablePlace,
        ExpressionNode::ArrayLiteral(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::Call(_)
        | ExpressionNode::Cast(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::StructLiteral(_)
        | ExpressionNode::String(_) => RuntimeTextSource::OtherExpression,
    }
}

fn output_buffer_target_for_text_expression(
    host_calls: &HostCallPlan,
    plan: &mut RuntimeTextPlan,
    expression: ExpressionHandle,
) -> Option<ExpressionHandle> {
    let source_expression = host_calls.expressions.expression(expression);
    if matches!(source_expression, ExpressionNode::Mutable(_)) {
        return None;
    }

    plan.buffers
        .iter()
        .find(|(_, buffer)| {
            buffer.text_place.is_valid()
                && expression_place_eq_across_tables(
                    &plan.expressions,
                    buffer.text_place,
                    &host_calls.expressions,
                    expression,
                )
        })
        .map(|(_, buffer)| buffer.target)
}
