use crate::InstructionSelectionInput;
use crate::selection::bindings::{
    RuntimeAliasBinding, resolve_runtime_alias_expression, strip_mutable_expression,
};
use crate::selection::host_operations::runtime_text_input_buffer_data_for_text_place;
use crate::selection::storage_places::{
    resolve_runtime_frame_indexed_target, resolve_runtime_pointee_slot_offset,
    resolve_runtime_storage_place, resolve_runtime_storage_place_in_table,
};
use omega_checked_trees::expression::{Expression, ExpressionTable};
use omega_control_flow::StateKey;
use omega_runtime_text::RuntimeTextBuilderSegmentKind;
use omega_target_operations::{
    RuntimeStorageRegion, SelectedInstructionKind, TargetDataObjectHandle,
};

#[allow(clippy::too_many_arguments)]
pub(in crate::selection) fn runtime_text_builder_write_emit(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    source_machine: &str,
    source_state: &str,
    statement_index: usize,
    resolved_target: &Expression,
    aliases: &[RuntimeAliasBinding],
    alias_expressions: &ExpressionTable,
    emit: &mut dyn FnMut(SelectedInstructionKind),
) -> bool {
    if aliases.is_empty()
        && runtime_text_builder_write_without_aliases_emit(
            input,
            dispatch_index,
            source_key,
            source_machine,
            source_state,
            statement_index,
            resolved_target,
            emit,
        )
    {
        return true;
    }

    runtime_text_builder_write_with_resolver_emit(
        input,
        dispatch_index,
        source_key,
        source_machine,
        source_state,
        statement_index,
        resolved_target,
        &|expression| {
            resolve_runtime_alias_expression(expression, source_key, aliases, alias_expressions)
        },
        emit,
    )
}

#[allow(clippy::too_many_arguments)]
fn runtime_text_builder_write_without_aliases_emit(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    source_machine: &str,
    source_state: &str,
    statement_index: usize,
    resolved_target: &Expression,
    emit: &mut dyn FnMut(SelectedInstructionKind),
) -> bool {
    let Some(builder) = input
        .runtime_text
        .builders
        .iter()
        .find(|(_, builder)| {
            builder.source_key == source_key && builder.statement_index == statement_index
        })
        .map(|(_, builder)| builder)
    else {
        return false;
    };
    let Some(segments) = input.runtime_text.builder_segments.span(builder.segments) else {
        return false;
    };
    let resolved_target = strip_mutable_expression(resolved_target.clone());
    let buffer = runtime_text_input_buffer_data_for_text_place(input, &resolved_target);
    if !buffer.is_valid() {
        return false;
    };
    let target_place = resolve_runtime_storage_place(
        input,
        dispatch_index,
        source_key,
        source_machine,
        source_state,
        &resolved_target,
    );
    let target_indexed =
        resolve_runtime_frame_indexed_target(input, dispatch_index, source_key, &resolved_target);
    let target_pointee =
        resolve_runtime_pointee_slot_offset(input, dispatch_index, source_key, &resolved_target);
    if target_place.is_none() && target_pointee.is_none() && target_indexed.is_none() {
        return false;
    }
    if target_place
        .as_ref()
        .is_some_and(|place| place.byte_count != input.target.pointer_size * 2)
    {
        return false;
    }
    if target_pointee
        .as_ref()
        .is_some_and(|target| target.pointee_byte_size != input.target.pointer_size * 2)
    {
        return false;
    }
    if target_indexed
        .as_ref()
        .is_some_and(|target| target.byte_count != input.target.pointer_size * 2)
    {
        return false;
    }

    if let [prefix, suffix] = segments
        && prefix.kind == RuntimeTextBuilderSegmentKind::StaticText
        && suffix.kind == RuntimeTextBuilderSegmentKind::StoredPlace
        && let Some(target_place) = target_place.as_ref()
    {
        return prefixed_stored_place_write_without_aliases(
            input,
            dispatch_index,
            source_key,
            buffer,
            target_place.region,
            target_place.byte_offset,
            prefix,
            suffix,
            emit,
        );
    }

    let mut emitted = false;
    for segment in segments {
        match segment.kind {
            RuntimeTextBuilderSegmentKind::StoredPlace => {
                let Some(source_place) = resolve_runtime_storage_place_in_table(
                    input,
                    dispatch_index,
                    source_key,
                    &input.runtime_text.expressions,
                    segment.expression,
                ) else {
                    return false;
                };
                if source_place.byte_count != input.target.pointer_size * 2 {
                    return false;
                }
                if let Some(target_place) = target_place.as_ref() {
                    if source_place.region == target_place.region
                        && source_place.byte_offset == target_place.byte_offset
                    {
                        emit(SelectedInstructionKind::MaterializeRuntimeTextBuffer {
                            buffer,
                            target_region: target_place.region,
                            target_offset: target_place.byte_offset,
                        });
                        emitted = true;
                        continue;
                    }
                    emit(SelectedInstructionKind::AppendRuntimeTextStoredPlace {
                        buffer,
                        source_region: source_place.region,
                        source_offset: source_place.byte_offset,
                        target_region: target_place.region,
                        target_offset: target_place.byte_offset,
                    });
                } else if let Some(target) = target_pointee.as_ref() {
                    emit(
                        SelectedInstructionKind::AppendRuntimeTextStoredPlaceToRuntimePointee {
                            buffer,
                            source_region: source_place.region,
                            source_offset: source_place.byte_offset,
                            pointer_byte_offset: target.pointer_byte_offset,
                            field_byte_offset: target.field_byte_offset,
                        },
                    );
                } else if let Some(target) = target_indexed.as_ref() {
                    emit(SelectedInstructionKind::AppendRuntimeTextStoredPlaceToRuntimeFrameIndexed {
                        buffer,
                        source_region: source_place.region,
                        source_offset: source_place.byte_offset,
                        descriptor_offset: target.descriptor_offset,
                        index_offset: target.index_offset,
                        element_byte_size: target.element_byte_size,
                        field_byte_offset: target.field_byte_offset,
                    });
                } else {
                    return false;
                }
                emitted = true;
            }
            RuntimeTextBuilderSegmentKind::StaticText => {
                let Some(literal) = input
                    .runtime_text
                    .expressions
                    .string_literal_value(segment.expression)
                else {
                    return false;
                };
                if let Some(target_place) = target_place.as_ref() {
                    emit(SelectedInstructionKind::AppendRuntimeTextLiteral {
                        buffer,
                        target_region: target_place.region,
                        target_offset: target_place.byte_offset,
                        literal,
                    });
                } else if let Some(target) = target_pointee.as_ref() {
                    emit(
                        SelectedInstructionKind::AppendRuntimeTextLiteralToRuntimePointee {
                            buffer,
                            pointer_byte_offset: target.pointer_byte_offset,
                            field_byte_offset: target.field_byte_offset,
                            literal,
                        },
                    );
                } else if let Some(target) = target_indexed.as_ref() {
                    emit(
                        SelectedInstructionKind::AppendRuntimeTextLiteralToRuntimeFrameIndexed {
                            buffer,
                            descriptor_offset: target.descriptor_offset,
                            index_offset: target.index_offset,
                            element_byte_size: target.element_byte_size,
                            field_byte_offset: target.field_byte_offset,
                            literal,
                        },
                    );
                } else {
                    return false;
                }
                emitted = true;
            }
            RuntimeTextBuilderSegmentKind::OtherExpression => return false,
        }
    }

    emitted
}

#[allow(clippy::too_many_arguments)]
pub(in crate::selection) fn runtime_text_builder_write_with_resolver_emit(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    source_machine: &str,
    source_state: &str,
    statement_index: usize,
    resolved_target: &Expression,
    resolve_expression: &dyn Fn(&Expression) -> Expression,
    emit: &mut dyn FnMut(SelectedInstructionKind),
) -> bool {
    let Some(builder) = input
        .runtime_text
        .builders
        .iter()
        .find(|(_, builder)| {
            builder.source_key == source_key && builder.statement_index == statement_index
        })
        .map(|(_, builder)| builder)
    else {
        return false;
    };
    let Some(segments) = input.runtime_text.builder_segments.span(builder.segments) else {
        return false;
    };
    let resolved_target = strip_mutable_expression(resolved_target.clone());
    let buffer = runtime_text_input_buffer_data_for_text_place(input, &resolved_target);
    if !buffer.is_valid() {
        return false;
    };
    let target_place = resolve_runtime_storage_place(
        input,
        dispatch_index,
        source_key,
        source_machine,
        source_state,
        &resolved_target,
    );
    let target_indexed =
        resolve_runtime_frame_indexed_target(input, dispatch_index, source_key, &resolved_target);
    let target_pointee =
        resolve_runtime_pointee_slot_offset(input, dispatch_index, source_key, &resolved_target);
    if target_place.is_none() && target_pointee.is_none() && target_indexed.is_none() {
        return false;
    }
    if target_place
        .as_ref()
        .is_some_and(|place| place.byte_count != input.target.pointer_size * 2)
    {
        return false;
    }
    if target_pointee
        .as_ref()
        .is_some_and(|target| target.pointee_byte_size != input.target.pointer_size * 2)
    {
        return false;
    }
    if target_indexed
        .as_ref()
        .is_some_and(|target| target.byte_count != input.target.pointer_size * 2)
    {
        return false;
    }

    if let [prefix, suffix] = segments
        && prefix.kind == RuntimeTextBuilderSegmentKind::StaticText
        && suffix.kind == RuntimeTextBuilderSegmentKind::StoredPlace
        && let Some(target_place) = target_place.as_ref()
    {
        return prefixed_stored_place_write(
            input,
            dispatch_index,
            source_key,
            source_machine,
            source_state,
            buffer,
            target_place.region,
            target_place.byte_offset,
            prefix,
            suffix,
            resolve_expression,
            emit,
        );
    }

    let mut emitted = false;
    for segment in segments {
        match segment.kind {
            RuntimeTextBuilderSegmentKind::StoredPlace => {
                let segment_expression = input.runtime_text.expressions.to_tree(segment.expression);
                let source = resolve_expression(&segment_expression);
                let Some(source_place) = resolve_runtime_storage_place(
                    input,
                    dispatch_index,
                    source_key,
                    source_machine,
                    source_state,
                    &source,
                ) else {
                    return false;
                };
                if source_place.byte_count != input.target.pointer_size * 2 {
                    return false;
                }
                if let Some(target_place) = target_place.as_ref() {
                    if source_place.region == target_place.region
                        && source_place.byte_offset == target_place.byte_offset
                    {
                        emit(SelectedInstructionKind::MaterializeRuntimeTextBuffer {
                            buffer,
                            target_region: target_place.region,
                            target_offset: target_place.byte_offset,
                        });
                        emitted = true;
                        continue;
                    }
                    emit(SelectedInstructionKind::AppendRuntimeTextStoredPlace {
                        buffer,
                        source_region: source_place.region,
                        source_offset: source_place.byte_offset,
                        target_region: target_place.region,
                        target_offset: target_place.byte_offset,
                    });
                } else if let Some(target) = target_pointee.as_ref() {
                    emit(
                        SelectedInstructionKind::AppendRuntimeTextStoredPlaceToRuntimePointee {
                            buffer,
                            source_region: source_place.region,
                            source_offset: source_place.byte_offset,
                            pointer_byte_offset: target.pointer_byte_offset,
                            field_byte_offset: target.field_byte_offset,
                        },
                    );
                } else if let Some(target) = target_indexed.as_ref() {
                    emit(SelectedInstructionKind::AppendRuntimeTextStoredPlaceToRuntimeFrameIndexed {
                        buffer,
                        source_region: source_place.region,
                        source_offset: source_place.byte_offset,
                        descriptor_offset: target.descriptor_offset,
                        index_offset: target.index_offset,
                        element_byte_size: target.element_byte_size,
                        field_byte_offset: target.field_byte_offset,
                    });
                } else {
                    return false;
                }
                emitted = true;
            }
            RuntimeTextBuilderSegmentKind::StaticText => {
                let Some(literal) = input
                    .runtime_text
                    .expressions
                    .string_literal_value(segment.expression)
                else {
                    return false;
                };
                if let Some(target_place) = target_place.as_ref() {
                    emit(SelectedInstructionKind::AppendRuntimeTextLiteral {
                        buffer,
                        target_region: target_place.region,
                        target_offset: target_place.byte_offset,
                        literal,
                    });
                } else if let Some(target) = target_pointee.as_ref() {
                    emit(
                        SelectedInstructionKind::AppendRuntimeTextLiteralToRuntimePointee {
                            buffer,
                            pointer_byte_offset: target.pointer_byte_offset,
                            field_byte_offset: target.field_byte_offset,
                            literal,
                        },
                    );
                } else if let Some(target) = target_indexed.as_ref() {
                    emit(
                        SelectedInstructionKind::AppendRuntimeTextLiteralToRuntimeFrameIndexed {
                            buffer,
                            descriptor_offset: target.descriptor_offset,
                            index_offset: target.index_offset,
                            element_byte_size: target.element_byte_size,
                            field_byte_offset: target.field_byte_offset,
                            literal,
                        },
                    );
                } else {
                    return false;
                }
                emitted = true;
            }
            RuntimeTextBuilderSegmentKind::OtherExpression => return false,
        }
    }

    emitted
}

#[allow(clippy::too_many_arguments)]
fn prefixed_stored_place_write(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    source_machine: &str,
    source_state: &str,
    buffer: TargetDataObjectHandle,
    target_region: RuntimeStorageRegion,
    target_offset: usize,
    prefix: &omega_runtime_text::RuntimeTextBuilderSegment,
    suffix: &omega_runtime_text::RuntimeTextBuilderSegment,
    resolve_expression: &dyn Fn(&Expression) -> Expression,
    emit: &mut dyn FnMut(SelectedInstructionKind),
) -> bool {
    let Some(prefix) = input
        .runtime_text
        .expressions
        .string_literal_value(prefix.expression)
    else {
        return false;
    };
    let suffix_expression = input.runtime_text.expressions.to_tree(suffix.expression);
    let source = resolve_expression(&suffix_expression);
    let Some(source_place) = resolve_runtime_storage_place(
        input,
        dispatch_index,
        source_key,
        source_machine,
        source_state,
        &source,
    ) else {
        return false;
    };
    if source_place.byte_count != input.target.pointer_size * 2 {
        return false;
    }

    emit(SelectedInstructionKind::WriteRuntimeTextLiteralSegment {
        buffer,
        byte_offset: 0,
        literal: prefix.clone(),
    });
    emit(SelectedInstructionKind::AppendRuntimeTextStoredSuffix {
        buffer,
        buffer_offset: prefix.len(),
        source_region: source_place.region,
        source_offset: source_place.byte_offset,
        target_region,
        target_offset,
        length_delta: prefix.len(),
    });
    true
}

#[allow(clippy::too_many_arguments)]
fn prefixed_stored_place_write_without_aliases(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    buffer: TargetDataObjectHandle,
    target_region: RuntimeStorageRegion,
    target_offset: usize,
    prefix: &omega_runtime_text::RuntimeTextBuilderSegment,
    suffix: &omega_runtime_text::RuntimeTextBuilderSegment,
    emit: &mut dyn FnMut(SelectedInstructionKind),
) -> bool {
    let Some(prefix) = input
        .runtime_text
        .expressions
        .string_literal_value(prefix.expression)
    else {
        return false;
    };
    let Some(source_place) = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        source_key,
        &input.runtime_text.expressions,
        suffix.expression,
    ) else {
        return false;
    };
    if source_place.byte_count != input.target.pointer_size * 2 {
        return false;
    }

    emit(SelectedInstructionKind::WriteRuntimeTextLiteralSegment {
        buffer,
        byte_offset: 0,
        literal: prefix.clone(),
    });
    emit(SelectedInstructionKind::AppendRuntimeTextStoredSuffix {
        buffer,
        buffer_offset: prefix.len(),
        source_region: source_place.region,
        source_offset: source_place.byte_offset,
        target_region,
        target_offset,
        length_delta: prefix.len(),
    });
    true
}
