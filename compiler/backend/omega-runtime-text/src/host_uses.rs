use omega_calling_conventions::PlatformCallData;
use omega_platform_interface::{HostCall, HostCallArgumentKind, HostCallPlan};
use omega_typed_trees::expression::{Expression, NamePath};
use omega_typed_trees::name::ProgramName;

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
        let expression_handle = plan.expressions.insert_tree(expression);
        let source = classify_runtime_text_source(expression);
        plan.uses.insert(RuntimeTextUse {
            source_key: host_call.source_key,
            statement_index: host_call.statement_index,
            platform_call: host_call.platform_call.clone(),
            expression: expression_handle,
            source,
            append_newline,
        });

        if source == RuntimeTextSource::StoredPlace
            && let Some(target) = output_buffer_target_for_text_expression(expression)
        {
            plan.buffers.insert(RuntimeTextBuffer {
                source_key: host_call.source_key,
                statement_index: host_call.statement_index,
                platform_call: host_call.platform_call.clone(),
                target: plan.expressions.insert_tree(&target),
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

    let HostCallArgumentKind::Expression(Expression::Mutable(target)) = &first_argument.kind else {
        return;
    };

    plan.buffers.insert(RuntimeTextBuffer {
        source_key: host_call.source_key,
        statement_index: host_call.statement_index,
        platform_call: host_call.platform_call.clone(),
        target: plan.expressions.insert_tree(target),
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

fn classify_runtime_text_source(expression: &Expression) -> RuntimeTextSource {
    match expression {
        Expression::Name(_) | Expression::Indexed(_) | Expression::Member(_) => {
            RuntimeTextSource::StoredPlace
        }
        Expression::Binary(_) => RuntimeTextSource::GeneratedString,
        Expression::Mutable(_) => RuntimeTextSource::MutablePlace,
        Expression::ArrayLiteral(_)
        | Expression::Boolean(_)
        | Expression::Call(_)
        | Expression::Cast(_)
        | Expression::Float(_)
        | Expression::Integer(_)
        | Expression::StructLiteral(_)
        | Expression::String(_) => RuntimeTextSource::OtherExpression,
    }
}

fn output_buffer_target_for_text_expression(expression: &Expression) -> Option<Expression> {
    match expression {
        Expression::Member(member) if member.member == "text" => Some(member.receiver.clone()),
        Expression::Name(path)
            if path.len() > 1 && path.last().is_some_and(|member| member.as_str() == "text") =>
        {
            let members = path
                .members()
                .iter()
                .take(path.len() - 1)
                .cloned()
                .collect::<Vec<ProgramName>>();
            Some(Expression::Name(NamePath::resolved(
                members,
                path.head_symbol(),
                path.head_symbol(),
            )))
        }
        Expression::Mutable(inner) => output_buffer_target_for_text_expression(inner)
            .map(|target| Expression::Mutable(Box::new(target))),
        _ => None,
    }
}
