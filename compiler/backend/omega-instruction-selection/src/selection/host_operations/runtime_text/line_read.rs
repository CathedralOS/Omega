use super::bindings::host_binding_mechanism;
use crate::InstructionSelectionInput;
use crate::selection::bindings::{RuntimeAliasResolutionContext, resolve_runtime_alias_binding};
use crate::selection::storage_places::{
    RuntimeStoragePlace, resolve_runtime_storage_place, resolve_runtime_storage_place_in_table,
};
use omega_calling_conventions::{
    HostBindingMechanism, HostCapability, HostOperation, HostOperationKey, PlatformCallData,
};
use omega_checked_trees::expression::{
    Expression, ExpressionHandle, ExpressionNode, ExpressionTable, NamePath,
};
use omega_platform_interface::HostCall;
use omega_target_operations::{RuntimeTextReadSource, SelectedInstructionKind};

pub(in crate::selection::host_operations) fn runtime_text_line_read(
    input: &InstructionSelectionInput<'_>,
    host_call: &HostCall,
    dispatch_index: Option<u32>,
    _alias_context: Option<RuntimeAliasResolutionContext<'_, '_>>,
) -> Option<SelectedInstructionKind> {
    let PlatformCallData::MutableOutputBuffer { byte_capacity } = host_call.data else {
        return None;
    };
    let Some(read_source) = runtime_text_read_source(input) else {
        return None;
    };

    let buffer = input
        .runtime_text
        .buffers
        .iter()
        .find(|(_, buffer)| {
            buffer.source_key == host_call.source_key
                && buffer.statement_index == host_call.statement_index
        })
        .map(|(_, buffer)| buffer)?;
    let (data_object, _) = input
        .data
        .objects
        .iter()
        .find(|(_, data_object)| {
            data_object.source_key == buffer.source_key
                && data_object.source_statement == buffer.statement_index
        })
        .map(|(data, data_object)| (data, data_object))?;
    let target_place = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index.unwrap_or(0),
        host_call.source_key,
        &input.runtime_text.expressions,
        buffer.text_place,
    )?;
    if target_place.byte_count != input.target.pointer_size * 2 {
        return None;
    }

    Some(SelectedInstructionKind::ReadRuntimeTextLine {
        buffer: data_object,
        target_region: target_place.region,
        target_offset: target_place.byte_offset,
        byte_capacity,
        source: read_source,
    })
}

fn runtime_text_read_source(
    input: &InstructionSelectionInput<'_>,
) -> Option<RuntimeTextReadSource> {
    match host_binding_mechanism(
        input,
        HostOperationKey::new(HostCapability::Stdin, HostOperation::Read),
    )? {
        HostBindingMechanism::Import { .. } => Some(RuntimeTextReadSource::Import {
            operation_key: HostOperationKey::new(HostCapability::Stdin, HostOperation::Read),
        }),
        HostBindingMechanism::Syscall {
            number,
            number_register,
            supervisor_call,
            ..
        } => Some(RuntimeTextReadSource::Syscall {
            number: *number,
            number_register: *number_register,
            supervisor_call: *supervisor_call,
        }),
    }
}

pub(in crate::selection) fn runtime_string_descriptor_place(
    input: &InstructionSelectionInput<'_>,
    host_call: &HostCall,
    dispatch_index: Option<u32>,
    alias_context: Option<RuntimeAliasResolutionContext<'_, '_>>,
) -> Option<RuntimeStoragePlace> {
    let first_argument = input
        .host_calls
        .arguments
        .span(host_call.arguments)
        .and_then(|arguments| arguments.first())?;
    let omega_platform_interface::HostCallArgumentKind::Expression(expression) =
        &first_argument.kind
    else {
        return None;
    };

    if alias_context.is_none()
        && !host_call_argument_has_alias(
            input,
            host_call.source_key,
            &input.host_calls.expressions,
            *expression,
        )
        && let Some(place) = resolve_runtime_storage_place_in_table(
            input,
            dispatch_index.unwrap_or(0),
            host_call.source_key,
            &input.host_calls.expressions,
            *expression,
        )
    {
        return (place.byte_count == input.target.pointer_size * 2).then_some(place);
    }

    let expression = input.host_calls.expressions.to_tree(*expression);
    let (resolved_source_key, resolved_expression) = alias_context
        .map(|context| {
            let resolved = resolve_runtime_alias_binding(
                &expression,
                host_call.source_key,
                context.aliases,
                context.alias_expressions,
            );
            (resolved.source_key, resolved.expression)
        })
        .unwrap_or((host_call.source_key, expression));
    let (resolved_source_key, resolved_expression) =
        resolve_host_call_alias_expression(input, resolved_source_key, &resolved_expression);
    let source_machine = input
        .control_flow
        .state_machine_name_by_key_cloned(resolved_source_key);
    let source_state = input
        .control_flow
        .state_name_by_key_cloned(resolved_source_key);
    let place = resolve_runtime_storage_place(
        input,
        dispatch_index.unwrap_or(0),
        resolved_source_key,
        &source_machine,
        &source_state,
        &resolved_expression,
    )?;

    (place.byte_count == input.target.pointer_size * 2).then_some(place)
}

fn host_call_argument_has_alias(
    input: &InstructionSelectionInput<'_>,
    source_key: omega_control_flow::StateKey,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> bool {
    match expressions.expression(expression) {
        ExpressionNode::Mutable(target) => {
            host_call_argument_has_alias(input, source_key, expressions, *target)
        }
        ExpressionNode::Indexed(indexed) => {
            host_call_argument_has_alias(input, source_key, expressions, indexed.collection)
        }
        ExpressionNode::Member(member) => {
            host_call_argument_has_alias(input, source_key, expressions, member.receiver)
        }
        ExpressionNode::Name(path) => input.alias_flow.aliases.iter().any(|(_, alias)| {
            alias.callee_key == source_key
                && alias.parameter_symbol.is_valid()
                && path.head_symbol.is_valid()
                && alias.parameter_symbol == path.head_symbol
        }),
        _ => false,
    }
}

fn resolve_host_call_alias_expression(
    input: &InstructionSelectionInput<'_>,
    source_key: omega_control_flow::StateKey,
    expression: &Expression,
) -> (omega_control_flow::StateKey, Expression) {
    match expression {
        Expression::Mutable(target) => {
            let (resolved_source_key, resolved_target) =
                resolve_host_call_alias_expression(input, source_key, target);
            (
                resolved_source_key,
                Expression::Mutable(Box::new(resolved_target)),
            )
        }
        Expression::Indexed(indexed) => {
            let (resolved_source_key, resolved_collection) =
                resolve_host_call_alias_expression(input, source_key, &indexed.collection);
            let (_, resolved_index) =
                resolve_host_call_alias_expression(input, source_key, &indexed.index);
            (
                resolved_source_key,
                Expression::Indexed(Box::new(
                    omega_checked_trees::expression::IndexedExpression {
                        collection: resolved_collection,
                        index: resolved_index,
                    },
                )),
            )
        }
        Expression::Member(member) => {
            let (resolved_source_key, resolved_receiver) =
                resolve_host_call_alias_expression(input, source_key, &member.receiver);
            (
                resolved_source_key,
                Expression::Member(Box::new(
                    omega_checked_trees::expression::MemberExpression {
                        receiver: resolved_receiver,
                        member_symbol: member.member_symbol,
                        member: member.member.clone(),
                    },
                )),
            )
        }
        Expression::Name(path) if !path.is_empty() => {
            let mut matched_alias = None;
            for (_, alias) in input.alias_flow.aliases.iter() {
                if alias.callee_key == source_key && alias_matches_path(alias, path) {
                    matched_alias = Some(alias);
                }
            }

            matched_alias
                .map(|alias| {
                    let aliased_expression = input
                        .alias_flow
                        .expressions
                        .to_tree_with_place_suffix(alias.argument, &path[1..]);
                    resolve_host_call_alias_expression(input, alias.caller_key, &aliased_expression)
                })
                .unwrap_or((source_key, expression.clone()))
        }
        _ => (source_key, expression.clone()),
    }
}

fn alias_matches_path(alias: &omega_state_calls::AliasBinding, path: &NamePath) -> bool {
    alias.parameter_symbol.is_valid()
        && path.head_symbol().is_valid()
        && alias.parameter_symbol == path.head_symbol()
}
