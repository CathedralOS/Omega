use omega_calling_conventions::PlatformCallData;
use omega_checked_trees::expression::{ExpressionHandle, ExpressionNode, TableNamePath};
use omega_core::arena::HandleSpan;
use omega_core::symbols::SymbolHandle;
use omega_platform_interface::{HostCall, HostCallArgumentKind, HostCallPlan};

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
            platform_call: host_call.platform_call.clone(),
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
                platform_call: host_call.platform_call.clone(),
                target,
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

    plan.buffers.insert(RuntimeTextBuffer {
        source_key: host_call.source_key,
        statement_index: host_call.statement_index,
        platform_call: host_call.platform_call.clone(),
        target: plan.expressions.copy_from(&host_calls.expressions, *target),
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
    match host_calls.expressions.expression(expression) {
        ExpressionNode::Member(member) if member.member.as_str() == "text" => Some(
            plan.expressions
                .copy_from(&host_calls.expressions, member.receiver),
        ),
        ExpressionNode::Name(path) => {
            let source_members = host_calls.expressions.name_path_members(path.members);
            if source_members.len() <= 1
                || !source_members
                    .last()
                    .is_some_and(|member| member.as_str() == "text")
            {
                return None;
            }

            let mut members = HandleSpan::empty();
            let mut member_symbols = HandleSpan::empty();
            let source_member_symbols = host_calls
                .expressions
                .name_path_member_symbols(path.member_symbols);
            for member in source_members.iter().take(source_members.len() - 1) {
                plan.expressions
                    .push_name_path_member(&mut members, member.clone());
            }
            for (offset, _) in source_members
                .iter()
                .take(source_members.len() - 1)
                .enumerate()
            {
                let member_symbol = source_member_symbols
                    .get(offset)
                    .copied()
                    .unwrap_or_else(SymbolHandle::invalid);
                plan.expressions
                    .push_name_path_member_symbol(&mut member_symbols, member_symbol);
            }
            Some(plan.expressions.insert(ExpressionNode::Name(TableNamePath {
                members,
                member_symbols,
                head_symbol: path.head_symbol,
                symbol: path.head_symbol,
            })))
        }
        ExpressionNode::Mutable(inner) => {
            let target = output_buffer_target_for_text_expression(host_calls, plan, *inner)?;
            Some(plan.expressions.insert(ExpressionNode::Mutable(target)))
        }
        _ => None,
    }
}
