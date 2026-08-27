use crate::InstructionSelectionInput;
use crate::selection::bindings::{
    RuntimeAliasBuffer, RuntimeAliasResolutionContext, resolve_runtime_alias_binding_handle,
};
use crate::selection::storage_places::{
    RuntimeStoragePlace, resolve_runtime_storage_place_in_table,
    resolve_runtime_storage_place_is_bounded_byte_buffer_in_table,
    resolve_runtime_storage_place_is_fixed_byte_array_in_table,
};
use omega_abstract_operations::{RuntimeTextReadTarget, SelectedInstructionKind};
use omega_calling_conventions::PlatformCallData;
use omega_platform_interface::HostCall;
use psi_checked_trees::expression::{
    ExpressionHandle, ExpressionNode, ExpressionTable, TableBorrowExpression,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::selection) struct RuntimeStringDescriptorPlace {
    pub(in crate::selection) place: RuntimeStoragePlace,
    pub(in crate::selection) through_pointee: bool,
    /// The place is an owned `[u8; N]` carrier (`{len, bytes}` inline), not a
    /// `{ptr, len}` descriptor: host-call content addressing reads `len` at offset
    /// 0 and uses `place + pointer_size` as the content pointer.
    pub(in crate::selection) is_bounded_buffer: bool,
}

pub(in crate::selection::host_operations) fn runtime_text_line_read(
    input: &InstructionSelectionInput<'_>,
    host_call: &HostCall,
    dispatch_index: Option<u32>,
    _alias_context: Option<RuntimeAliasResolutionContext<'_, '_>>,
) -> Option<SelectedInstructionKind> {
    let PlatformCallData::MutableOutputBuffer { byte_capacity } = host_call.data else {
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
    // Owned carriers and raw byte arrays read straight into their inline bytes;
    // a String stays on the detached-buffer `{ptr, len}` path.
    let is_bounded_buffer = resolve_runtime_storage_place_is_bounded_byte_buffer_in_table(
        input,
        dispatch_index.unwrap_or(0),
        host_call.source_key,
        &input.runtime_text.expressions,
        buffer.text_place,
    );
    let is_fixed_byte_array = resolve_runtime_storage_place_is_fixed_byte_array_in_table(
        input,
        dispatch_index.unwrap_or(0),
        host_call.source_key,
        &input.runtime_text.expressions,
        buffer.text_place,
    );
    let target = if is_bounded_buffer {
        RuntimeTextReadTarget::BoundedByteBuffer
    } else if is_fixed_byte_array {
        RuntimeTextReadTarget::FixedByteArray
    } else if target_place.byte_count == input.runtime_abi.string_descriptor_size() {
        RuntimeTextReadTarget::StringDescriptor
    } else {
        return None;
    };
    // The host ABI's mutable-output capacity describes the legacy detached
    // String scratch buffer. An owned `[u8; N]` carrier is the destination
    // itself, so its inline capacity is authoritative: using the legacy limit
    // here would allow a short carrier to be overwritten by a longer line.
    let byte_capacity = match target {
        RuntimeTextReadTarget::BoundedByteBuffer => target_place
            .byte_count
            .checked_sub(input.runtime_abi.pointer_size)?,
        RuntimeTextReadTarget::FixedByteArray => target_place.byte_count,
        RuntimeTextReadTarget::StringDescriptor => byte_capacity,
    };

    Some(SelectedInstructionKind::ReadRuntimeTextLine {
        buffer: data_object,
        target_region: target_place.region,
        target_offset: target_place.byte_offset,
        byte_capacity,
        target,
    })
}

pub(in crate::selection) fn runtime_string_descriptor_place(
    input: &InstructionSelectionInput<'_>,
    host_call: &HostCall,
    dispatch_index: Option<u32>,
    alias_context: Option<RuntimeAliasResolutionContext<'_, '_>>,
) -> Option<RuntimeStringDescriptorPlace> {
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
        let is_bounded_buffer = resolve_runtime_storage_place_is_bounded_byte_buffer_in_table(
            input,
            dispatch_index.unwrap_or(0),
            host_call.source_key,
            &input.host_calls.expressions,
            *expression,
        );
        return runtime_string_descriptor_from_place(input, place, is_bounded_buffer);
    }

    let mut expressions = ExpressionTable::with_expression_capacity(
        alias_context
            .map(|context| context.aliases.len())
            .unwrap_or(0)
            .saturating_add(4),
    );
    let copied_aliases = alias_context.map(|context| {
        RuntimeAliasBuffer::copy_from_bindings(
            context.alias_expressions,
            context.aliases,
            &mut expressions,
        )
    });
    let expression_handle = expressions.copy_from(&input.host_calls.expressions, *expression);
    let (resolved_source_key, resolved_expression) = copied_aliases
        .as_ref()
        .map(|aliases| {
            let resolved = resolve_runtime_alias_binding_handle(
                expression_handle,
                host_call.source_key,
                aliases.bindings(),
                &mut expressions,
            );
            (resolved.source_key, resolved.expression)
        })
        .unwrap_or((host_call.source_key, expression_handle));
    let (resolved_source_key, resolved_expression) = resolve_host_call_alias_expression_handle(
        input,
        resolved_source_key,
        &mut expressions,
        resolved_expression,
    );
    if let Some(place) = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index.unwrap_or(0),
        resolved_source_key,
        &expressions,
        resolved_expression,
    ) {
        let is_bounded_buffer = resolve_runtime_storage_place_is_bounded_byte_buffer_in_table(
            input,
            dispatch_index.unwrap_or(0),
            resolved_source_key,
            &expressions,
            resolved_expression,
        );
        return runtime_string_descriptor_from_place(input, place, is_bounded_buffer);
    }

    None
}

fn runtime_string_descriptor_from_place(
    input: &InstructionSelectionInput<'_>,
    place: RuntimeStoragePlace,
    is_bounded_buffer: bool,
) -> Option<RuntimeStringDescriptorPlace> {
    // An owned `[u8; N]` carrier owns its bytes inline -- take it directly with
    // carrier addressing, BEFORE the size checks below (its `{len, bytes}` size
    // can collide with the 16-byte descriptor size, e.g. `[u8; 8]`).
    if is_bounded_buffer {
        return Some(RuntimeStringDescriptorPlace {
            place,
            through_pointee: false,
            is_bounded_buffer: true,
        });
    }
    if place.byte_count == input.runtime_abi.string_descriptor_size() {
        return Some(RuntimeStringDescriptorPlace {
            place,
            through_pointee: false,
            is_bounded_buffer: false,
        });
    }
    if place.byte_count == input.runtime_abi.pointer_size {
        return Some(RuntimeStringDescriptorPlace {
            place,
            through_pointee: true,
            is_bounded_buffer: false,
        });
    }
    None
}

fn host_call_argument_has_alias(
    input: &InstructionSelectionInput<'_>,
    source_key: omega_control_flow::StateKey,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> bool {
    match expressions.expression(expression) {
        ExpressionNode::Borrow(target) => {
            host_call_argument_has_alias(input, source_key, expressions, target.target)
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

fn resolve_host_call_alias_expression_handle(
    input: &InstructionSelectionInput<'_>,
    source_key: omega_control_flow::StateKey,
    expressions: &mut ExpressionTable,
    expression: ExpressionHandle,
) -> (omega_control_flow::StateKey, ExpressionHandle) {
    match expressions.expression(expression).clone() {
        ExpressionNode::Borrow(target) => {
            let (resolved_source_key, resolved_target) = resolve_host_call_alias_expression_handle(
                input,
                source_key,
                expressions,
                target.target,
            );
            (
                resolved_source_key,
                expressions.insert(ExpressionNode::Borrow(TableBorrowExpression {
                    target: resolved_target,
                    access: target.access,
                })),
            )
        }
        ExpressionNode::Indexed(indexed) => {
            let (resolved_source_key, resolved_collection) =
                resolve_host_call_alias_expression_handle(
                    input,
                    source_key,
                    expressions,
                    indexed.collection,
                );
            let (_, resolved_index) = resolve_host_call_alias_expression_handle(
                input,
                source_key,
                expressions,
                indexed.index,
            );
            (
                resolved_source_key,
                expressions.insert(ExpressionNode::Indexed(
                    psi_checked_trees::expression::TableIndexedExpression {
                        collection: resolved_collection,
                        index: resolved_index,
                    },
                )),
            )
        }
        ExpressionNode::Member(member) => {
            let (resolved_source_key, resolved_receiver) =
                resolve_host_call_alias_expression_handle(
                    input,
                    source_key,
                    expressions,
                    member.receiver,
                );
            (
                resolved_source_key,
                expressions.insert(ExpressionNode::Member(
                    psi_checked_trees::expression::TableMemberExpression {
                        receiver: resolved_receiver,
                        member_symbol: member.member_symbol,
                        member: member.member,
                        case_variant: member.case_variant,
                    },
                )),
            )
        }
        ExpressionNode::Name(path) => {
            let mut matched_alias = None;
            for (_, alias) in input.alias_flow.aliases.iter() {
                if alias.callee_key == source_key && alias_matches_table_path(alias, &path) {
                    matched_alias = Some(alias);
                }
            }

            matched_alias
                .map(|alias| {
                    let argument =
                        expressions.copy_from(&input.alias_flow.expressions, alias.argument);
                    let aliased_expression = if path.members.count() > 0 {
                        expressions.insert_copy_with_member_suffix(
                            argument,
                            path.members,
                            path.member_symbols,
                            1,
                        )
                    } else {
                        argument
                    };
                    resolve_host_call_alias_expression_handle(
                        input,
                        alias.caller_key,
                        expressions,
                        aliased_expression,
                    )
                })
                .unwrap_or((source_key, expression))
        }
        _ => (source_key, expression),
    }
}

fn alias_matches_table_path(
    alias: &omega_state_calls::AliasBinding,
    path: &psi_checked_trees::expression::TableNamePath,
) -> bool {
    alias.parameter_symbol.is_valid()
        && path.head_symbol.is_valid()
        && alias.parameter_symbol == path.head_symbol
}
